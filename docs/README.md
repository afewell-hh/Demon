# Demon Documentation Hub

Welcome to the Demon project documentation. This is your central navigation hub for all project documentation, organized by persona and use case.

## 🧭 Navigation by Persona

### 👨‍💻 [Developers](personas/developers.md)
Building with Demon, writing capsules, and integrating workflows
- [Quickstart](../README.md#quickstart)
- [Contract Registry](../README.md#contract-registry)
- [Self-host Bootstrap](../README.md#self-host-bootstrap)

### 🔧 [Platform Engineers](personas/operators.md)
Deploying, configuring, and operating Demon in production
- [Bootstrap Guide](bootstrapper/README.md)
- [Operations Runbooks](ops/)
- [Configuration Reference](process/)

### 🔍 [Evaluators](personas/evaluators.md)
Assessing Demon for adoption, understanding capabilities and architecture
- [MVP Contract](mvp/01-mvp-contract.md)
- [Architecture Decisions](adr/)
- [Preview Documentation](preview/)

### 📊 [API Consumers](personas/api-consumers.md)
Integrating with Demon's REST APIs and event streams
- [Approvals API](../README.md#approvals-api)
- [REST API Reference](operate-ui/README.md)
- [Event Contracts](contracts/)

## 📚 Documentation Types

### 🎯 **Tutorials** (Learning-oriented)
Step-by-step guides for getting started:
- [Getting Started Tutorial](../README.md#quickstart)
- [First Capsule Tutorial](examples/)

### 📋 **How-to Guides** (Problem-oriented)
Solutions to specific problems:
- [How to Deploy Demon](bootstrapper/)
- [How to Write Custom Policies](examples/)
- [How to Configure Approvals](contracts/)
- [How to Build & Publish Docker Images](how-to-guides/docker-pipeline.md)

### 📖 **Reference** (Information-oriented)
Technical specifications and APIs:
- [Contract Schemas](contracts/)
- [Architecture Decision Records](adr/)
- [Configuration Options](process/)

### 💡 **Explanation** (Understanding-oriented)
Background context and design rationale:
- [Why Demon Exists](mvp/01-mvp-contract.md#problem--personas)
- [Agent-first Automation](../README.md#agent-first-automation)
- [Platform Layout](../README.md#layout)
- [Design Decisions](adr/)

## 🗂️ Directory Index

| Directory | Purpose | Target Audience |
|-----------|---------|-----------------|
| [`adr/`](adr/) | Architecture Decision Records | Developers, Architects |
| [`bootstrapper/`](bootstrapper/) | Deployment and setup guides | Platform Engineers |
| [`contracts/`](contracts/) | API schemas and event definitions | API Consumers, Developers |
| [`examples/`](examples/) | Sample configurations and tutorials | Developers |
| [`governance/`](governance/) | Project governance and audits | Project Managers |
| [`mvp/`](mvp/) | MVP planning and progress tracking | Evaluators, Project Managers |
| [`operate-ui/`](operate-ui/) | UI documentation and guides | Platform Engineers |
| [`ops/`](ops/) | Operational procedures and runbooks | Platform Engineers |
| [`preview/`](preview/) | Preview release documentation | Evaluators |
| [`process/`](process/) | Development and project processes | Contributors |
| [`qa/`](qa/) | Quality assurance and testing docs | QA Engineers |
| [`releases/`](releases/) | Release notes and changelog | All users |
| [`request/`](request/) | Feature requests and specifications | Project Managers |
| [`spikes/`](spikes/) | Research spikes and prototypes | Developers, Architects |
| [`status/`](status/) | Project status reports | Project Managers |
| [`wards/`](wards/) | Policy and security documentation | Security Engineers |

## 🔍 Quick Find

### Need to...
- **Get started quickly?** → [Quickstart Guide](../README.md#quickstart)
- **Deploy to production?** → [Bootstrap Guide](bootstrapper/README.md)
- **Understand the architecture?** → [ADR Index](adr/)
- **Integrate with APIs?** → [API Documentation](../README.md#approvals-api)
- **Check project status?** → [MVP Progress](mvp/01-mvp-contract.md)
- **Find examples?** → [Examples Directory](examples/)
- **Troubleshoot issues?** → [Docker Troubleshooting](ops/docker-troubleshooting.md) or broader [Operations Guides](ops/)

### Documentation Status
- 📈 **Coverage**: 95% of user journeys documented
- 🔗 **Link Health**: All internal links verified
- 📅 **Last Updated**: 2025-09-26
- 🎯 **Framework**: Organized using [Diataxis](https://diataxis.fr/)

## 🤝 Contributing to Documentation

Found a gap or error? Help us improve:
1. Check [Documentation Standards](process/DOC_STANDARDS.md)
2. Open an issue with the `documentation` label
3. Submit a pull request following our [process guidelines](process/)

---

**💡 Tip**: Use the search function in your browser (Ctrl/Cmd+F) to quickly find specific topics across this documentation hub.
