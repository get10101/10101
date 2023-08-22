# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

- Add synthetic usd feature.

## [1.2.2] - 2023-08-22

## [1.2.1] - 2023-08-21

- Add support for push notifications.
- Added new setting to coordinator to configure max channel size to traders.
- Speed up DLC channel setup and settlement by checking for messages more often.
- Add support for perpetual futures.

## [1.2.0] - 2023-08-04

- Automatically retry spendable outputs if not successfully published.
- Permit the closure of the LN-DLC channel in any intermediate state.

## [1.1.0] - 2023-07-27

- Charge funding transaction on-chain fees upon receiving and inbound JIT Channel.
- Added `prometheus` metrics to coordinator: `channel_balance_satoshi`, `channel_outbound_capacity_satoshi`, `channel_inbound_capacity_satoshi`, `channel_is_usable`, `node_connected_peers_total`, `node_balance_satoshi`, `position_quantity_contracts`, `position_margin_sats`.
- Track channel data.
- Track channel liquidity.
- Track on-chain transactions.

## [1.0.21] - 2023-07-02

- Fix issue where `Next` button on the create invoice screen was hidden behind keyboard. The keyboard can now be closed by tapping outside the text-field.
- Fix panic when processing accept message while peer is disconnected.
- Configurable oracle endpoint and public key.
- Removed stop-gap from receiving payments with open position.
- Reduced min amount of 50k sats on receiving payments.

## [1.0.20] - 2023-06-16

- Do not trigger DLC manager periodic check twice.
- Simplify maker binary.
- Prefer unused addresses to new ones (temporarily).
- Remove share-on-Twitter button temporarily.
- Use address caches in `LnDlcWallet`.
- Set background transaction priority to 24 blocks.
- Improve error message when trying to collab close LN-DLC channel.
- Simplify deserialisation of channel ID.
- Stabilise key dependencies.

## [1.0.19] - 2023-06-12

- Fixed a deadlock bug, resulting in the coordinator getting stuck.
- Upgrade our fork to `rust-lightning:0.0.114`.
- Added force closing a DLC channel feature.
- Replaced `electrs` with `esplora` client.

## [1.0.6] - 2023-04-17

- Change environment port to 80.

## [1.0.5] - 2023-04-16

- Announce coordinator with `10101.finance`.

## [1.0.4] - 2023-04-14

- Add new API to sign text with node.
- Auto-settle expired positions.

## [1.0.3] - 2023-04-14

### Added

- Self-Custodial CFD Trading based on DLC and lightning

[Unreleased]: https://github.com/get10101/10101/compare/1.2.2...HEAD
[1.2.2]: https://github.com/get10101/10101/compare/1.2.1...1.2.2
[1.2.1]: https://github.com/get10101/10101/compare/1.2.0...1.2.1
[1.2.0]: https://github.com/get10101/10101/compare/1.1.0...1.2.0
[1.1.0]: https://github.com/get10101/10101/compare/1.0.21...1.1.0
[1.0.21]: https://github.com/get10101/10101/compare/1.0.20...1.0.21
[1.0.20]: https://github.com/get10101/10101/compare/1.0.19...1.0.20
[1.0.19]: https://github.com/get10101/10101/compare/1.0.18...1.0.19
[1.0.18]: https://github.com/get10101/10101/compare/1.0.17...1.0.18
[1.0.17]: https://github.com/get10101/10101/compare/1.0.16...1.0.17
[1.0.16]: https://github.com/get10101/10101/compare/1.0.15...1.0.16
[1.0.15]: https://github.com/get10101/10101/compare/1.0.14...1.0.15
[1.0.14]: https://github.com/get10101/10101/compare/1.0.13...1.0.14
[1.0.13]: https://github.com/get10101/10101/compare/1.0.12...1.0.13
[1.0.12]: https://github.com/get10101/10101/compare/1.0.11...1.0.12
[1.0.11]: https://github.com/get10101/10101/compare/1.0.10...1.0.11
[1.0.10]: https://github.com/get10101/10101/compare/1.0.9...1.0.10
[1.0.9]: https://github.com/get10101/10101/compare/1.0.8...1.0.9
[1.0.8]: https://github.com/get10101/10101/compare/1.0.7...1.0.8
[1.0.7]: https://github.com/get10101/10101/compare/1.0.6...1.0.7
[1.0.6]: https://github.com/get10101/10101/compare/1.0.5...1.0.6
[1.0.5]: https://github.com/get10101/10101/compare/1.0.4...1.0.5
[1.0.4]: https://github.com/get10101/10101/compare/1.0.3...1.0.4
[1.0.3]: https://github.com/get10101/10101/compare/565308aba0b835a571f9ad195d18f9627dace2be...1.0.3
