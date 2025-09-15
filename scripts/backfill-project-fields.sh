#!/bin/bash
set -euo pipefail

# Script to backfill project fields for issues #56-#63
# Requires GH_TOKEN with project write permissions

echo "Starting backfill of project fields for issues #56-#63"

# Get the project ID (project number 1)
PROJECT_ID=$(gh api graphql -f query='
  query($owner: String!, $number: Int!) {
    user(login: $owner) {
      projectV2(number: $number) {
        id
      }
    }
  }' -f owner="afewell-hh" -F number=1 --jq '.data.user.projectV2.id')

if [[ -z "$PROJECT_ID" ]]; then
  echo "Error: Could not find project"
  exit 1
fi

echo "Found project ID: $PROJECT_ID"

# Get field IDs and options
FIELDS=$(gh api graphql -f query='
  query($projectId: ID!) {
    node(id: $projectId) {
      ... on ProjectV2 {
        fields(first: 20) {
          nodes {
            ... on ProjectV2Field {
              id
              name
            }
            ... on ProjectV2SingleSelectField {
              id
              name
              options {
                id
                name
              }
            }
          }
        }
      }
    }
  }' -f projectId="$PROJECT_ID")

# Extract field and option IDs
AREA_FIELD=$(echo "$FIELDS" | jq -r '.data.node.fields.nodes[] | select(.name == "Area") | .id')
PRIORITY_FIELD=$(echo "$FIELDS" | jq -r '.data.node.fields.nodes[] | select(.name == "Priority") | .id')
TARGET_FIELD=$(echo "$FIELDS" | jq -r '.data.node.fields.nodes[] | select(.name == "Target Release") | .id')

BACKEND_OPTION=$(echo "$FIELDS" | jq -r '.data.node.fields.nodes[] | select(.name == "Area") | .options[] | select(.name == "backend") | .id')
FRONTEND_OPTION=$(echo "$FIELDS" | jq -r '.data.node.fields.nodes[] | select(.name == "Area") | .options[] | select(.name == "frontend") | .id')
P0_OPTION=$(echo "$FIELDS" | jq -r '.data.node.fields.nodes[] | select(.name == "Priority") | .options[] | select(.name == "p0") | .id')
ALPHA_OPTION=$(echo "$FIELDS" | jq -r '.data.node.fields.nodes[] | select(.name == "Target Release") | .options[] | select(.name == "MVP-Alpha") | .id')

# Function to update a field value
update_field() {
  local item_id=$1
  local field_id=$2
  local option_id=$3
  local field_name=$4

  if [[ -n "$field_id" ]] && [[ -n "$option_id" ]]; then
    gh api graphql -f query='
      mutation($projectId: ID!, $itemId: ID!, $fieldId: ID!, $value: ProjectV2FieldValue!) {
        updateProjectV2ItemFieldValue(input: {
          projectId: $projectId
          itemId: $itemId
          fieldId: $fieldId
          value: $value
        }) {
          projectV2Item {
            id
          }
        }
      }' -f projectId="$PROJECT_ID" -f itemId="$item_id" -f fieldId="$field_id" -f value="{\"singleSelectOptionId\": \"$option_id\"}" >/dev/null
    echo "  ✓ Set $field_name"
  else
    echo "  ⚠ Could not set $field_name (field or option not found)"
  fi
}

# Backend issues: #56-#59
for issue_num in 56 57 58 59; do
  echo "Processing issue #$issue_num (backend)..."

  # Get issue node ID
  ISSUE_ID=$(gh api repos/afewell-hh/demon/issues/$issue_num --jq '.node_id')

  # Get project item ID for this issue
  ITEM_ID=$(gh api graphql -f query='
    query($projectId: ID!, $issueId: ID!) {
      node(id: $projectId) {
        ... on ProjectV2 {
          items(first: 100) {
            nodes {
              id
              content {
                ... on Issue {
                  id
                }
              }
            }
          }
        }
      }
    }' -f projectId="$PROJECT_ID" -f issueId="$ISSUE_ID" | jq -r --arg id "$ISSUE_ID" '.data.node.items.nodes[] | select(.content.id == $id) | .id')

  if [[ -z "$ITEM_ID" ]]; then
    echo "  ⚠ Issue not found in project, skipping"
    continue
  fi

  update_field "$ITEM_ID" "$AREA_FIELD" "$BACKEND_OPTION" "Area=backend"
  update_field "$ITEM_ID" "$PRIORITY_FIELD" "$P0_OPTION" "Priority=p0"
  update_field "$ITEM_ID" "$TARGET_FIELD" "$ALPHA_OPTION" "Target Release=MVP-Alpha"
done

# Frontend issues: #60-#63
for issue_num in 60 61 62 63; do
  echo "Processing issue #$issue_num (frontend)..."

  # Get issue node ID
  ISSUE_ID=$(gh api repos/afewell-hh/demon/issues/$issue_num --jq '.node_id')

  # Get project item ID for this issue
  ITEM_ID=$(gh api graphql -f query='
    query($projectId: ID!, $issueId: ID!) {
      node(id: $projectId) {
        ... on ProjectV2 {
          items(first: 100) {
            nodes {
              id
              content {
                ... on Issue {
                  id
                }
              }
            }
          }
        }
      }
    }' -f projectId="$PROJECT_ID" -f issueId="$ISSUE_ID" | jq -r --arg id "$ISSUE_ID" '.data.node.items.nodes[] | select(.content.id == $id) | .id')

  if [[ -z "$ITEM_ID" ]]; then
    echo "  ⚠ Issue not found in project, skipping"
    continue
  fi

  update_field "$ITEM_ID" "$AREA_FIELD" "$FRONTEND_OPTION" "Area=frontend"
  update_field "$ITEM_ID" "$PRIORITY_FIELD" "$P0_OPTION" "Priority=p0"
  update_field "$ITEM_ID" "$TARGET_FIELD" "$ALPHA_OPTION" "Target Release=MVP-Alpha"
done

echo "Backfill complete!"