# Solana MEV Bot - Pool Creation Sniper
A high-performance MEV bot for detecting and executing trades on Solana pool creations.

## Overview
This bot monitors pool creations across two environments:
- **Development**: pump.fun program with test parameters
- **Production**: DAO.FUN program with live trading parameters

## Quick Start

### Environment Setup
```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Install Solana CLI
sh -c "$(curl -sSfL https://release.solana.com/v1.17.0/install)"

# Clone and Build
git clone [repository-url]
cd solana-mev-bot
cargo build
```

## Development Workflow

### 1. Local Development Setup
```bash
# Create development wallet
solana-keygen new --outfile dev_wallet.json

# Configure development environment
export MEV_ENV=development
```

### Development Parameters
```toml
[execution]
purchase_amount = 250000000    # 0.25 SOL
jito_tip = 10000000           # 0.01 SOL
slippage_percentage = 2.0
program_id = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA" # pump.fun
```

### Testing
```bash
# Run test suite
cargo test

# Run with development config
cargo run --features development
```

## Production Workflow

### 1. Production Deployment Setup
```bash
# Secure wallet creation
solana-keygen new --outfile prod_wallet.json --no-bip39-passphrase

# Configure production environment
export MEV_ENV=production
```

### Production Parameters
```toml
[execution]
purchase_amount = 100000000000 # 100 SOL
jito_tip = 250000000          # 0.25 SOL
slippage_percentage = 2.0
program_id = "5jnapfrAN47UYkLkEf7HnprPPBCQLvkYWGZDeKkaP5hv" # DAO.FUN
```

### Security Measures
```bash
# Set up monitoring
./setup_monitoring.sh

# Configure alerts
./configure_alerts.sh
```

## Project Structure
```
├── src/
│   ├── config/          # Configuration management
│   ├── core/            # Core bot functionality
│   │   ├── websocket.rs # Real-time monitoring
│   │   └── execution.rs # Trade execution
│   └── utils/           # Utility functions
├── scripts/             # Deployment & maintenance scripts
└── monitoring/          # Metrics & alerting
```

## Key Features

### Development Environment
- Monitors pump.fun program
- Test trades with 0.25 SOL
- Verifies >220 token holders
- Local logging and monitoring

### Production Environment
- Monitors DAO.FUN program
- Live trades with 100 SOL
- Enhanced security measures
- Production-grade monitoring

## Core Functionality

### Pool Detection
```rust
// Both environments monitor CreateIdempotent
impl PoolMonitor {
    async fn monitor_pools(&self) -> Result<()> {
        match self.config.environment {
            Environment::Development => {
                // Monitor pump.fun
                // Verify 220+ holders
            },
            Environment::Production => {
                // Monitor DAO.FUN
                // Enhanced security checks
            }
        }
    }
}
```

### Trade Execution
```rust
// Shared execution logic with environment-specific parameters
impl TransactionExecutor {
    async fn execute_buy(&self) -> Result<()> {
        let params = match self.config.environment {
            Environment::Development => BuyParams {
                amount: 0.25 * LAMPORTS_PER_SOL,
                max_slippage: 0.02,
                priority_fee: 0.01 * LAMPORTS_PER_SOL,
            },
            Environment::Production => BuyParams {
                amount: 100 * LAMPORTS_PER_SOL,
                max_slippage: 0.02,
                priority_fee: 0.25 * LAMPORTS_PER_SOL,
            },
        };
        
        self.execute_with_params(params).await
    }
}
```

## Monitoring & Metrics

### Development Metrics
- Transaction success rate
- Execution timing
- Token holder verification
- Local error logging

### Production Metrics
- Real-time performance monitoring
- Financial metrics tracking
- System health monitoring
- Alert system integration

## Deployment Process

### Development Deployment
```bash
# Local deployment
cargo run --features development

# Monitor logs
tail -f development.log
```

### Production Deployment
```bash
# Security checks
./security_audit.sh

# Deploy
cargo build --release --features production
systemctl start mev-bot

# Monitor
./monitor_production.sh
```

## Maintenance Procedures

### Development Maintenance
- Local backup creation
- Test suite execution
- Configuration updates
- Performance testing

### Production Maintenance
```bash
# Regular health checks
./health_check.sh

# Backup procedure
./backup_production.sh

# Performance monitoring
./monitor_metrics.sh
```

## Common Tasks

### Development Tasks
1. Adding new detection logic
2. Updating test parameters
3. Local performance testing
4. Configuration testing

### Production Tasks
1. Security updates
2. Performance optimization
3. Monitoring adjustment
4. Alert management

## Best Practices

### Development
- Test all changes locally first
- Maintain comprehensive test coverage
- Document configuration changes
- Monitor execution times

### Production
- Regular security audits
- Continuous monitoring
- Backup management
- Alert response procedures

## Troubleshooting

### Common Issues
1. RPC Connection Issues
   - Check network connectivity
   - Verify RPC endpoint status
   - Review API key validity

2. Websocket Disconnections
   - Implement automatic reconnection
   - Monitor connection stability
   - Log disconnection events

3. Transaction Failures
   - Check wallet balance
   - Verify slippage settings
   - Review priority fees

### Debug Tools
```bash
# Check logs
tail -f /var/log/mev-bot/error.log

# Monitor transactions
solana transaction-history

# Verify RPC connection
curl -X POST -H "Content-Type: application/json" -d '{"jsonrpc":"2.0","id":1,"method":"getHealth"}' http://your-rpc-endpoint
```

## Contributing
1. Fork the repository
2. Create your feature branch
3. Commit your changes
4. Push to the branch
5. Create a new Pull Request