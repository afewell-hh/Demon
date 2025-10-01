# Documentation Standards

This document defines the standards and guidelines for creating and maintaining documentation in the Demon project.

## Overview

Good documentation is essential for project success. These standards ensure consistency, clarity, and maintainability across all documentation artifacts.

## Framework

We use the **[Diataxis](https://diataxis.fr/)** framework to organize documentation:

- **Tutorials** (Learning-oriented) - Step-by-step lessons for beginners
- **How-to Guides** (Problem-oriented) - Solutions to specific problems
- **Reference** (Information-oriented) - Technical specifications and APIs
- **Explanation** (Understanding-oriented) - Background context and design rationale

## Documentation Types

### README Files
Every directory with public documentation MUST have a README.md file that includes:

- **Purpose** - What this directory contains
- **Overview** - Brief summary of contents
- **Quick Navigation** - Links to key documents
- **Usage Guidelines** - How to use the documentation

#### README Template
```markdown
# [Directory Name]

Brief description of purpose and contents.

## Overview
[2-3 sentences explaining what's in this directory]

## Key Documents
- [Document Name](file.md) - Brief description
- [Another Document](file2.md) - Brief description

## Quick Start
[Most common use case with example]

---
**🔗 Related**: [Link to related docs]
```

### API Documentation
All APIs MUST be documented with:

- **Endpoint specification** - URL, method, parameters
- **Request/response examples** - Working code samples
- **Error handling** - Status codes and error formats
- **Authentication** - Security requirements
- **Rate limiting** - Usage constraints

### Architecture Documents
Architecture Decision Records (ADRs) MUST follow this format:

```markdown
# ADR-XXXX: [Decision Title]

## Status
[Proposed | Accepted | Deprecated | Superseded]

## Context
[Background and problem statement]

## Decision
[What we decided to do and why]

## Consequences
[Positive and negative outcomes]

## Alternatives Considered
[Other options evaluated]
```

### Process Documentation
Process documents MUST include:

- **Purpose** - Why this process exists
- **Scope** - What it covers and excludes
- **Steps** - Clear, actionable procedures
- **Roles** - Who does what
- **Tools** - Required tools and access
- **Examples** - Working demonstrations

## Navigation & Status Conventions

### Document Status Indicators
All documents MUST include a status badge near the title to indicate their current state:

```markdown
# Document Title

![Status: Current](https://img.shields.io/badge/Status-Current-green)

Document content starts here...
```

**Available Status Badges:**
- `![Status: Current](https://img.shields.io/badge/Status-Current-green)` - Up-to-date, actively maintained
- `![Status: Draft](https://img.shields.io/badge/Status-Draft-yellow)` - Work in progress, may be incomplete
- `![Status: Deprecated](https://img.shields.io/badge/Status-Deprecated-red)` - No longer maintained, see alternatives

**Usage Guidelines:**
- **Current**: Default for all production documentation
- **Draft**: Use during development, include expected completion date
- **Deprecated**: Include link to replacement or migration path

### Breadcrumb Navigation
Long-form documents (tutorials, how-to guides, explanations) MUST include breadcrumb navigation:

```markdown
# Document Title

**📍 [Home](../../README.md) › [Tutorials](../README.md) › Document Title**

![Status: Current](https://img.shields.io/badge/Status-Current-green)

Document content starts here...
```

**Breadcrumb Rules:**
- Always start with Home link to docs root
- Include intermediate category (Tutorials, How-to Guides, etc.)
- End with current document title (not linked)
- Use consistent emoji indicators: 📍 for breadcrumbs, 🔗 for related links

## Writing Standards

### Language and Tone
- **Use active voice** - "Deploy the application" not "The application should be deployed"
- **Be concise** - Prefer shorter sentences and paragraphs
- **Use plain language** - Avoid jargon when simpler words work
- **Be specific** - Use concrete examples and precise instructions
- **Stay positive** - Focus on what to do, not what not to do

### Structure and Format
- **Use consistent headings** - Follow hierarchical structure (H1 → H2 → H3)
- **Include navigation** - Table of contents for long documents
- **Add quick reference** - Summary sections for frequently accessed info
- **Provide examples** - Code samples and working demonstrations
- **Link liberally** - Connect related concepts and documents

### Markdown Standards
- **Use semantic headings** - H1 for title, H2 for major sections, H3 for subsections
- **Format code consistently** - Use `inline code` and ```code blocks``` appropriately
- **Include line breaks** - Separate paragraphs and sections clearly
- **Use consistent lists** - Either `-` or `*` for bullets, `1.` for numbers
- **Link correctly** - Use relative paths for internal links

## Content Guidelines

### Code Examples
All code examples MUST:

- **Be working code** - Test all examples before publishing
- **Include context** - Show complete examples, not fragments
- **Explain purpose** - Comment on what the code does
- **Handle errors** - Show error cases and handling
- **Use real data** - Avoid "foo/bar" unless appropriate

#### Code Example Template
```bash
# Description of what this command does
command --flag value

# Expected output:
# Expected result shown here

# If it fails, check:
# - Common troubleshooting tips
# - Links to related documentation
```

### Tutorials
Tutorials MUST:

- **Start from zero** - Assume no prior knowledge
- **Be linear** - Follow logical step-by-step progression
- **Include checkpoints** - Verify progress at each step
- **Show expected results** - What success looks like
- **Handle failure** - What to do when things go wrong

### Reference Material
Reference documentation MUST:

- **Be comprehensive** - Cover all options and parameters
- **Stay current** - Update with code changes
- **Use consistent format** - Standard layout for similar content
- **Include examples** - Show usage for all major features
- **Cross-reference** - Link to related concepts

## Maintenance Standards

### Review Process
All documentation changes MUST:

- **Follow PR process** - No direct commits to main branch
- **Include review** - At least one reviewer for accuracy
- **Test examples** - Verify all code and instructions work
- **Run link checker** - Execute `./scripts/check-doc-links.sh` before submitting PR
- **Check links** - Ensure all internal links are valid
- **Update indexes** - Modify navigation and TOCs as needed

**Note**: GitHub Actions automatically runs link checking on documentation PRs (currently non-blocking during rollout).

### Update Triggers
Documentation MUST be updated when:

- **Code changes** - API modifications, new features, bug fixes
- **Process changes** - New procedures or policy updates
- **User feedback** - Issues or suggestions from users
- **Regular review** - Quarterly documentation audit
- **Breaking changes** - Major version updates

### Quality Metrics
We track documentation quality through:

- **Link health** - Automated checking for broken links
- **Coverage** - Percentage of features documented
- **Freshness** - How recently docs were updated
- **User feedback** - Issues and improvement suggestions
- **Usage analytics** - Most/least accessed content

## Tools and Automation

### Required Tools
- **Markdown linter** - Ensure consistent formatting
- **Link checker** - Automated broken link detection (see `scripts/check-doc-links.sh`)
- **Spell checker** - Catch typos and errors
- **Vale or similar** - Style and tone consistency

#### Link Checker Usage
The project includes a comprehensive link checking script:

```bash
# Install dependency
npm install -g markdown-link-check

# Check all documentation (internal links only)
./scripts/check-doc-links.sh

# Check including external links
./scripts/check-doc-links.sh --external

# Check specific directory
./scripts/check-doc-links.sh docs/tutorials

# Quiet mode for CI
./scripts/check-doc-links.sh --quiet

# Continue on errors (don't exit)
./scripts/check-doc-links.sh --continue
```

The script automatically creates a configuration file (`.markdown-link-check.json`) with sensible defaults and provides detailed reporting of broken links.

### Automation
- **Pre-commit hooks** - Run checks before commits
- **CI integration** - Automated testing in pull requests
- **Link validation** - Regular scanning for broken links
- **Dead code detection** - Find unused documentation

### Templates
Standard templates are available for:

- [README files](#readme-template)
- [ADR format](#architecture-documents)
- [See Also sections](#see-also-template)

#### See Also Template
Use this template to create consistent cross-references between related documents:

```markdown
## See Also

- [Tutorials](../tutorials/) - Learning-oriented guides for beginners
- [How-to Guides](../how-to-guides/) - Problem-solving oriented guides
- [Reference](../reference/) - Information-oriented documentation
- [Explanation](../explanation/) - Understanding-oriented discussions
- [Learning Paths](../getting-started/learning-paths.md) - Structured learning tracks

[← Back to Documentation Home](../README.md)
```

**Guidelines for See Also sections:**
- **Always include** - Every document should have cross-references
- **Use relative paths** - Link to other Diataxis categories appropriately
- **Be selective** - Include 3-5 most relevant links, not exhaustive lists
- **Include descriptions** - Brief phrase explaining what the link contains
- **End with navigation** - Always provide a way back to the main documentation
- **Order logically** - Start with most closely related content

## Persona-Specific Guidelines

### For Developers
- **Focus on how-to** - Practical guidance over theory
- **Include working examples** - Copy-pasteable code
- **Show common patterns** - Standard approaches and idioms
- **Link to reference** - Deep technical specifications

### For Operators
- **Emphasize procedures** - Step-by-step operational guidance
- **Include troubleshooting** - Common problems and solutions
- **Show monitoring** - What to watch and when to act
- **Provide escalation** - When and how to get help

### For Evaluators
- **Lead with value** - Benefits and capabilities first
- **Include comparisons** - How it differs from alternatives
- **Show proof points** - Concrete evidence of claims
- **Provide next steps** - Clear path forward

### For API Consumers
- **Show complete flows** - End-to-end integration examples
- **Document edge cases** - Error conditions and handling
- **Include rate limits** - Performance and usage constraints
- **Provide SDKs** - Language-specific guidance

## Review Checklist

Before publishing documentation, verify:

- [ ] **Purpose is clear** - Reader understands why this exists
- [ ] **Audience is obvious** - Target persona is evident
- [ ] **Structure is logical** - Information flows naturally
- [ ] **Examples work** - All code and instructions tested
- [ ] **Links are valid** - Internal and external links function
- [ ] **Style is consistent** - Follows project conventions
- [ ] **Grammar is correct** - Spell-checked and proofread
- [ ] **Navigation exists** - Document is discoverable
- [ ] **Maintenance plan** - Update triggers identified
- [ ] **Feedback mechanism** - Way for users to improve docs

## Continuous Improvement

### Feedback Collection
- **GitHub issues** - Documentation-specific issue template
- **Analytics** - Track usage patterns and drop-offs
- **User interviews** - Direct feedback from personas
- **Team retrospectives** - Internal improvement opportunities

### Regular Reviews
- **Quarterly audits** - Comprehensive review of all docs
- **Feature alignment** - Ensure docs match current capabilities
- **Link validation** - Automated and manual link checking
- **Freshness review** - Update stale or outdated content

### Metrics and Goals
- **Coverage target** - 95% of user journeys documented
- **Freshness target** - No docs older than 6 months without review
- **Link health** - Zero broken internal links
- **User satisfaction** - Positive feedback and low issue rates

---

**📝 Contributing**: Follow these standards when creating or updating documentation. When in doubt, ask for review.

**🔗 Related**: [Contributing Guidelines](../../CONTRIBUTING.md) | [Diataxis Framework](https://diataxis.fr/) | [Markdown Guide](https://www.markdownguide.org/)