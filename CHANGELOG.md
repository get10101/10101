# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

- Chore: move telegram link into toplevel of settings so that it can be found easier
- Feat: update coordinator API to show more details on pending channel balance
- Feat: show dlc-channel balance instead of ln-balance in app and in coordinator's API

## [1.7.4] - 2023-12-20

- Fix: allow recovering from a stuck position by rejecting or accepting pending offers

## [1.7.3] - 2023-12-13

- Fix: a bug which may lead to a stuck position due to some async tasks. Instead of having an async task closing positions we are explicitly closing positions now according to the protocol.
- Feat: Add delete network graph in settings
- Fix: Make switch buttons in receive screen not jumpy

## [1.7.2] - 2023-12-13

- Feat: Receive USD-P via Lightning

## [1.7.1] - 2023-12-08

- Feat: Pay lightning invoice with USD-P balance.
- Feat: Open scanner on send button.
- Feat: Add bidirectional swap drawer.
- Fix: Add route prober api.
- Feat: Do not arbitrarily cap routing fee, to increase likelihood of payment success.
- Fix: Only display active liquidity options.
- Chore: Reduce log output related to syncing the Lighting wallet.
- Fix: Revert dropping io-util in ldk 117 fork.

## [1.7.0] - 2023-12-07

- Fix: Reduce WebSocket reconnect timeout to 200ms.
- Feat: Replace speed dial button with send, receive and swap buttons.
- Chore: Log channel details on a failed payment attempt.
- Feat: Redesign of the wallet balance.

## [1.6.6] - 2023-12-01

- Fix: Add backwards-compatibility of `ChainMonitor`s created before version 1.5.0
- Fix: Return lsp config data in authenticated response
- Fix: Make function pieces continuous

## [1.6.5] - 2023-11-29

- Add support for Android 9
- Fix handle error on channel receive
- Add settings option to enable trace logs
- Add contract details to dlc channels api
- Add support for parsing invoices from Zeus
- Add social media links to app info

## [1.6.4] - 2023-11-24

- Fix build arguments to include rapid gossip sync server url

## [1.6.3] - 2023-11-24

- Add support for rapid gossip sync

## [1.6.2] - 2023-11-23

- Fix pnl calculation when resizing
- Add collab revert support without channel

## [1.6.1] - 2023-11-21

- Fix missing context when opening alert dialog
- Fix jagged border on carousel images
- Compute trade cost correctly in app wallet history

## [1.6.0] - 2023-11-20

- Move backup button to settings
- Refresh lightning wallet on received payment event
- Move thermostat status to settings
- Spawn blocking on force-close and close channel
- Spawn backup tasks from static tokio runtime
- Spawn blocking on send payment
- Return to the correct screen after returning from settings
- Increase back arrow clickable space on settings
- Do not call periodic check twice

## [1.5.1] - 2023-11-16

- Add on-boarding wizard
- Add backup and restore for lightning, dlc and 10101 data
- Add support for resizing positions
- Add beta disclaimer
- Ensure that payout curve cannot be negative
- Consider fee in payout curve
- Ensure that inverse payout curve does not go over oracle's maximum BTC price
- Add multiple oracle support on coordinator

## [1.5.0] - 2023-11-06

- Allow to drain on-chain wallet by sending amount `0`.
- Load persisted `rust-dlc` `ChainMonitor` on restart.
- Upgrade `rust-lightning` to version 0.0.116.
- Charge channel opening fee through the lsp flow
- Allow to configure tx fee rate when opening channels from the coordinator

## [1.4.4] - 2023-10-28

- Improve collab revert
- Improved settings screen
- Add recover from seed phrase. Note: for now we are only able to recover on-chain funds.

## [1.4.3] - 2023-10-23

- Charge order matching fee through the margin
- Improve wallet balance design
- Fix a bug where a position remained open even though the channel has been force closed.
- Fix a bug where the app got stuck when opening the receive screen.

## [1.4.2] - 2023-10-18

- Only contracts are editable for new orders
- Contracts can only be entered in full amounts
- New default amount is 10 contracts
- Fix: Show pnl on success dialog when closing a position
- Add a feature to collaboratively close a (potentially broken) channel
- Receive bitcoin in wallet
- Send bitcoin in wallet to address or BIP21 URI
- Redesign send and receive screens
- Add stable flag to position and order
- Add QR code scanner

## [1.4.1] - 2023-10-09

- Make trade / stable screen default if user has a position
- Also show app logs in production

## [1.4.0] - 2023-10-06

- Allow up to 5BTC wumbo channels
- Add push notification reminder that the rollover window is open now.
- Add liquidity options to the onboaring flow
- Pay fixed channel opening fees instead of onchain transaction fees
- Increased maximum channel size
- Add minimum channel size
- Split onboading and create invoice flow
- Format amounts in input fields
- Return to wallet dashboard on claimed payment

## [1.3.0] - 2023-09-27

- Find match on an expired position
- Show loading screen when app starts with an expired position
- Fix: Prevent crashing the app when there's no Internet connection
- feat: Allow exporting the seed phrase even if the Node is offline
- Changed expiry to next Sunday 3 pm UTC
- Automatically rollover if user opens app during rollover weekend
- Sync position with dlc channel state
- Extend coordinator's `/api/version` with commit hash and number
- When pulling down wallet sync it waits until the sync is finished
- Add close button to dialog if it remains on pending for more than 30 seconds

## [1.2.6] - 2023-09-06

## [1.2.5] - 2023-09-06

- Improve wallet sync speed by 100x (Use self-hosted esplora node)

## [1.2.4] - 2023-08-31

- Display Synthetic USD balance in the account overview screen.

## [1.2.3] - 2023-08-28

- Add synthetic USD feature.
- Fix delayed position update.
- Change contract duration to 7 days.
- Add settings in coordinator to make contract fee rate configurable during runtime.
- Fix: the status bar on iOS is always shown

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

[Unreleased]: https://github.com/get10101/10101/compare/1.7.4...HEAD
[1.7.4]: https://github.com/get10101/10101/compare/1.7.3...1.7.4
[1.7.3]: https://github.com/get10101/10101/compare/1.7.2...1.7.3
[1.7.2]: https://github.com/get10101/10101/compare/1.7.1...1.7.2
[1.7.1]: https://github.com/get10101/10101/compare/1.7.0...1.7.1
[1.7.0]: https://github.com/get10101/10101/compare/1.6.6...1.7.0
[1.6.6]: https://github.com/get10101/10101/compare/1.6.5...1.6.6
[1.6.5]: https://github.com/get10101/10101/compare/1.6.4...1.6.5
[1.6.4]: https://github.com/get10101/10101/compare/1.6.3...1.6.4
[1.6.3]: https://github.com/get10101/10101/compare/1.6.2...1.6.3
[1.6.2]: https://github.com/get10101/10101/compare/1.6.1...1.6.2
[1.6.1]: https://github.com/get10101/10101/compare/1.6.0...1.6.1
[1.6.0]: https://github.com/get10101/10101/compare/1.5.1...1.6.0
[1.5.1]: https://github.com/get10101/10101/compare/1.5.0...1.5.1
[1.5.0]: https://github.com/get10101/10101/compare/1.4.4...1.5.0
[1.4.4]: https://github.com/get10101/10101/compare/1.4.3...1.4.4
[1.4.3]: https://github.com/get10101/10101/compare/1.4.2...1.4.3
[1.4.2]: https://github.com/get10101/10101/compare/1.4.1...1.4.2
[1.4.1]: https://github.com/get10101/10101/compare/1.4.0...1.4.1
[1.4.0]: https://github.com/get10101/10101/compare/1.3.0...1.4.0
[1.3.0]: https://github.com/get10101/10101/compare/1.2.6...1.3.0
[1.2.6]: https://github.com/get10101/10101/compare/1.2.5...1.2.6
[1.2.5]: https://github.com/get10101/10101/compare/1.2.4...1.2.5
[1.2.4]: https://github.com/get10101/10101/compare/1.2.3...1.2.4
[1.2.3]: https://github.com/get10101/10101/compare/1.2.2...1.2.3
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
