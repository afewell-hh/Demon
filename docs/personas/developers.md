# Developers Guide

Welcome, developers! This guide helps you build with Demon, create custom capsules, and integrate workflow automation into your applications.

## 🚀 Quick Start

New to Demon? Start here:

1. **[Get the code running](../../README.md#quickstart)** - 5-minute setup
2. **[Understand the basics](../mvp/01-mvp-contract.md)** - What Demon does
3. **[Run your first ritual](../examples/)** - See it in action

```bash
# Quick start commands
make dev
cargo run -p demonctl -- run examples/rituals/echo.yaml
```

## 🛠️ Development Workflows

### Building Capsules
- [Echo Capsule Example](../../capsules/echo/) - Simple "Hello World" capsule
- [Writing Custom Capsules](../examples/) - Build your own automation
- [Contract Specifications](../contracts/) - API schemas and event definitions

### Working with Contracts
- [Contract Registry](../../README.md#contract-registry) - Export and manage schemas
- [JSON Schema Reference](../contracts/) - Event and API specifications
- [WIT Definitions](../contracts/) - WebAssembly interface types

### Local Development
- [Development Environment Setup](../../README.md#self-host-bootstrap)
- [NATS JetStream Configuration](../../docker/dev/)
- [Testing and Debugging](../process/)

## 📋 How-to Guides

### Essential Tasks
- [How to write a ritual YAML](../examples/rituals/)
- [How to create approval gates](../contracts/)
- [How to handle events](../contracts/)
- [How to integrate with external APIs](../examples/)

### Advanced Topics
- [How to implement custom policies](../examples/)
- [How to extend the runtime](../../runtime/)
- [How to contribute to the engine](../../engine/)

## 📖 Reference Documentation

### APIs and Schemas
- [Event Contracts](../contracts/) - Complete event schema reference
- [Runtime API](../../runtime/) - Capsule runtime interface
- [CLI Reference](../../demonctl/) - Command-line tool documentation

### Architecture
- [System Architecture](../../README.md#layout) - High-level component overview
- [Architecture Decisions](../adr/) - Design rationale and trade-offs
- [Engine Internals](../../engine/) - How ritual interpretation works

## 🔧 Tools and Utilities

### Development Tools
- `demonctl` - Main CLI tool for running rituals and managing contracts
- `cargo` - Standard Rust toolchain for building and testing
- `make` - Development task automation

### Useful Commands
```bash
# Development workflow
make dev                    # Start NATS and build workspace
make test                   # Run all tests
make fmt                    # Format code
make lint                   # Run linter

# Contract management
cargo run -p demonctl -- contracts bundle              # Export contracts
cargo run -p demonctl -- contracts fetch-bundle       # Download latest
cargo run -p demonctl -- contracts bundle --format json --include-wit

# Running rituals
cargo run -p demonctl -- run examples/rituals/echo.yaml
```

## 🎯 Common Use Cases

### Integration Patterns
- **CI/CD Automation** - Trigger builds, deployments, and notifications
- **Approval Workflows** - Human gates in automated processes
- **Policy Enforcement** - Quota management and compliance checks
- **Event-Driven Architecture** - React to system events with automated responses

### Example Scenarios
- Deploy to production with approval gates
- Automated security scanning with policy decisions
- Multi-stage approval for sensitive operations
- Event-driven notifications and alerting

## 🧪 Testing and Quality

### Testing Your Code
- [Unit Testing Guide](../process/) - Test individual components
- [Integration Testing](../qa/) - End-to-end testing approaches
- [Smoke Testing](../qa/) - Quick validation techniques

### Code Quality
- [Coding Standards](../process/) - Rust conventions and best practices
- [Review Process](../process/) - Code review guidelines
- [Documentation Standards](../process/DOC_STANDARDS.md) - Writing good docs

## 🤝 Contributing

### Getting Involved
- [Contribution Guidelines](../process/) - How to contribute code
- [Issue Templates](../../.github/) - Reporting bugs and requesting features
- [Development Process](../process/) - Workflow and branch protection

### Community
- [Project Governance](../governance/) - How decisions are made
- [MVP Progress](../mvp/) - Current development status
- [Roadmap](../mvp/02-epics.md) - Planned features and epics

## 📚 Learning Resources

### Further Reading
- [Meta-PaaS Concepts](../adr/ADR-0001-meta-paas-scope.md) - Understanding the vision
- [Policy and Quotas](../adr/ADR-0003-wards-policy-and-approvals.md) - Security model
- [Bundle Library](../adr/ADR-0007-bundle-library-and-provenance.md) - Package management

### External Resources
- [Rust Programming Language](https://doc.rust-lang.org/book/)
- [NATS JetStream Documentation](https://docs.nats.io/jetstream)
- [WebAssembly Component Model](https://component-model.bytecodealliance.org/)

---

**💡 Need help?** Check our [troubleshooting guide](../ops/) or open an issue with the `question` label.