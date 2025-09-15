#!/usr/bin/env bash
set -euo pipefail

# Backfill Project V2 fields for issues, tolerant of Single-select or Text fields.
# Usage: GH_TOKEN=... ./scripts/backfill-project-fields.sh 56 57 58 59 60 61 62 63

if ! command -v gh >/dev/null; then
  echo "gh CLI is required" >&2
  exit 1
fi

OWNER=$(gh repo view --json owner -q .owner.login)
REPO=$(gh repo view --json name -q .name)
PROJ=$(gh api graphql -f query='query($login:String!){user(login:$login){projectV2(number:1){id}}}' -f login="$OWNER" -q .data.user.projectV2.id)

FIELDS=$(gh api graphql -F proj="$PROJ" -f query='query($proj:ID!){node(id:$proj){... on ProjectV2{fields(first:50){nodes{__typename ... on ProjectV2FieldCommon{id name dataType} ... on ProjectV2SingleSelectField{id name dataType options{id name}}}} items(first:200){nodes{id content{__typename ... on Issue{number}}}}}}}')

get_item_id() {
  local num=$1
  echo "$FIELDS" | jq -r --argjson n "$num" '.data.node.items.nodes[] | select(.content.number == $n) | .id'
}

get_field() {
  local name=$1
  echo "$FIELDS" | jq -r --arg n "$name" '.data.node.fields.nodes[] | select(.name == $n) | .id'
}

get_type() {
  local name=$1
  echo "$FIELDS" | jq -r --arg n "$name" '.data.node.fields.nodes[] | select(.name == $n) | .dataType'
}

get_option() {
  local name=$1 opt=$2
  echo "$FIELDS" | jq -r --arg n "$name" --arg o "$opt" '.data.node.fields.nodes[] | select(.name == $n) | (.options // [])[] | select(.name == $o) | .id'
}

set_single() {
  local item=$1 field=$2 option=$3
  gh api graphql -F proj="$PROJ" -F item="$item" -F field="$field" -F option="$option" \
    -f query='mutation($proj:ID!,$item:ID!,$field:ID!,$option:String!){updateProjectV2ItemFieldValue(input:{projectId:$proj,itemId:$item,fieldId:$field,value:{singleSelectOptionId:$option}}){projectV2Item{id}}}' >/dev/null
}

set_text() {
  local item=$1 field=$2 text=$3
  gh api graphql -F proj="$PROJ" -F item="$item" -F field="$field" -F text="$text" \
    -f query='mutation($proj:ID!,$item:ID!,$field:ID!,$text:String!){updateProjectV2ItemFieldValue(input:{projectId:$proj,itemId:$item,fieldId:$field,value:{text:$text}}){projectV2Item{id}}}' >/dev/null
}

STATUS_FIELD=$(get_field "Status")
TODO_OPT=$(get_option "Status" "Todo" || true)
AREA_FIELD=$(get_field "Area" || true)
AREA_TYPE=$(get_type "Area" || true)
PRIORITY_FIELD=$(get_field "Priority" || true)
PRIORITY_TYPE=$(get_type "Priority" || true)
TARGET_FIELD=$(get_field "Target Release" || true)
TARGET_TYPE=$(get_type "Target Release" || true)

if [[ $# -eq 0 ]]; then
  echo "No issue numbers provided; exiting" >&2
  exit 1
fi

for n in "$@"; do
  echo "Backfilling #$n"
  item=$(get_item_id "$n")
  if [[ -z "$item" ]]; then
    node=$(gh api repos/$OWNER/$REPO/issues/$n -q .node_id)
    item=$(gh api graphql -F proj="$PROJ" -F item="$node" -f query='mutation($proj:ID!,$item:ID!){addProjectV2ItemById(input:{projectId:$proj,contentId:$item}){item{id}}}' -q .data.addProjectV2ItemById.item.id)
  fi
  # Status
  if [[ -n "$STATUS_FIELD" && -n "$TODO_OPT" ]]; then
    set_single "$item" "$STATUS_FIELD" "$TODO_OPT" || true
  fi
  # Infer labels
  labels=$(gh api repos/$OWNER/$REPO/issues/$n -q '[.labels[].name]|join(" ")')
  area=""; pri=""
  [[ "$labels" =~ area:backend ]] && area="backend"
  [[ "$labels" =~ area:frontend ]] && area="frontend"
  [[ "$labels" =~ p0 ]] && pri="p0"
  [[ "$labels" =~ p1 ]] && pri="p1"
  # Area
  if [[ -n "$AREA_FIELD" ]]; then
    if [[ "$AREA_TYPE" == "SINGLE_SELECT" ]]; then
      opt=$(get_option "Area" "$area" || true)
      [[ -n "$opt" ]] && set_single "$item" "$AREA_FIELD" "$opt" || true
    elif [[ -n "$area" ]]; then
      set_text "$item" "$AREA_FIELD" "$area" || true
    fi
  fi
  # Priority
  if [[ -n "$PRIORITY_FIELD" ]]; then
    if [[ "$PRIORITY_TYPE" == "SINGLE_SELECT" ]]; then
      opt=$(get_option "Priority" "$pri" || true)
      [[ -n "$opt" ]] && set_single "$item" "$PRIORITY_FIELD" "$opt" || true
    elif [[ -n "$pri" ]]; then
      set_text "$item" "$PRIORITY_FIELD" "$pri" || true
    fi
  fi
  # Target Release default
  if [[ -n "$TARGET_FIELD" ]]; then
    if [[ "$TARGET_TYPE" == "SINGLE_SELECT" ]]; then
      opt=$(get_option "Target Release" "MVP-Alpha" || true)
      [[ -n "$opt" ]] && set_single "$item" "$TARGET_FIELD" "$opt" || true
    else
      set_text "$item" "$TARGET_FIELD" "MVP-Alpha" || true
    fi
  fi
done

echo "Done."

