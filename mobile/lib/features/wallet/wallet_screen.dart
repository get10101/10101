import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:flutter_speed_dial/flutter_speed_dial.dart';
import 'package:get_10101/common/amount_text.dart';
import 'package:get_10101/common/channel_status_notifier.dart';
import 'package:get_10101/common/fiat_text.dart';
import 'package:get_10101/features/trade/position_change_notifier.dart';
import 'package:get_10101/features/wallet/balance_row.dart';
import 'package:get_10101/features/wallet/receive_screen.dart';
import 'package:get_10101/features/wallet/domain/wallet_history.dart';
import 'package:get_10101/features/wallet/domain/wallet_type.dart';
import 'package:get_10101/features/wallet/onboarding/onboarding_screen.dart';
import 'package:get_10101/features/wallet/seed_screen.dart';
import 'package:get_10101/features/wallet/send/send_screen.dart';
import 'package:get_10101/features/wallet/wallet_change_notifier.dart';
import 'package:get_10101/features/wallet/wallet_theme.dart';
import 'package:get_10101/util/preferences.dart';
import 'package:get_10101/util/send_receive_icons.dart';
import 'package:go_router/go_router.dart';
import 'package:provider/provider.dart';

class WalletScreen extends StatefulWidget {
  static const route = "/wallet";
  static const label = "Wallet";

  const WalletScreen({Key? key}) : super(key: key);

  @override
  State<WalletScreen> createState() => _WalletScreenState();
}

class _WalletScreenState extends State<WalletScreen> {
  bool _isBalanceBreakdownOpen = false;
  Future<bool>? isUserSeedBackupConfirmed;

  @override
  void initState() {
    super.initState();
    isUserSeedBackupConfirmed = Preferences.instance.isUserSeedBackupConfirmed();
  }

  @override
  Widget build(BuildContext context) {
    final walletChangeNotifier = context.watch<WalletChangeNotifier>();

    final hasChannel = context.watch<ChannelStatusNotifier>().hasChannel();

    // For displaying synthetic USD balance
    final positionChangeNotifier = context.watch<PositionChangeNotifier>();

    WalletTheme theme = Theme.of(context).extension<WalletTheme>()!;

    SizedBox listBottomScrollSpace = const SizedBox(
      height: 100,
    );

    return Scaffold(
      body: RefreshIndicator(
        onRefresh: () async {
          await walletChangeNotifier.refreshWalletInfo();
          await walletChangeNotifier.waitForSyncToComplete();
        },
        child: Container(
          margin: const EdgeInsets.only(top: 7.0),
          padding: const EdgeInsets.symmetric(horizontal: 20),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              ExpansionPanelList(
                children: [
                  ExpansionPanel(
                    headerBuilder: (BuildContext context, bool isExpanded) {
                      return Row(
                        mainAxisAlignment: MainAxisAlignment.center,
                        children: [
                          // https://stackoverflow.com/a/70192038 - do not know if this is principled
                          const SizedBox(width: 64),
                          // ExpansionPanelList IconContainer size: end margin 8 + padding 16*2 + size 24),
                          Center(
                              child: walletChangeNotifier.syncing
                                  ? const Text(
                                      'Wallet syncing',
                                      style: TextStyle(
                                        fontWeight: FontWeight.bold,
                                        fontStyle: FontStyle.italic,
                                      ),
                                    )
                                  : Row(
                                      children: [
                                        AmountText(
                                            amount: walletChangeNotifier.total(),
                                            textStyle: const TextStyle(
                                                fontSize: 20.0, fontWeight: FontWeight.bold)),
                                        Visibility(
                                            visible:
                                                positionChangeNotifier.getStableUSDAmountInFiat() !=
                                                    0.0,
                                            child: Row(
                                              children: [
                                                const SizedBox(width: 5),
                                                const Text("+"),
                                                const SizedBox(width: 5),
                                                FiatText(
                                                    amount: positionChangeNotifier
                                                        .getStableUSDAmountInFiat(),
                                                    textStyle: const TextStyle(
                                                        fontSize: 20.0,
                                                        fontWeight: FontWeight.bold)),
                                              ],
                                            )),
                                      ],
                                    )),
                        ],
                      );
                    },
                    body: Container(
                      margin: const EdgeInsets.only(left: 20.0, right: 20.0, bottom: 8.0),
                      child: const Column(
                        children: [
                          BalanceRow(walletType: WalletType.lightning),
                          BalanceRow(walletType: WalletType.onChain),
                          BalanceRow(walletType: WalletType.stable),
                        ],
                      ),
                    ),
                    isExpanded: _isBalanceBreakdownOpen,
                  )
                ],
                expansionCallback: (i, isOpen) => setState(() => _isBalanceBreakdownOpen = isOpen),
              ),
              const SizedBox(height: 10.0),
              if (walletChangeNotifier.lightning().sats == 0)
                ElevatedButton(
                  onPressed: () {
                    context.go(OnboardingScreen.route);
                  },
                  child: const Text("Fund Wallet"),
                ),
              FutureBuilder(
                  future: isUserSeedBackupConfirmed,
                  builder: (BuildContext context, AsyncSnapshot<bool> snapshot) {
                    // FIXME: We ignore the value of `isUserSeedBackupConfirmed` stored in
                    // `snapshot.data` to keep the `Backup Wallet` button visible at all times for
                    // now. We need to rework this.
                    if (snapshot.connectionState == ConnectionState.done) {
                      return Column(
                        crossAxisAlignment: CrossAxisAlignment.stretch,
                        children: [
                          const SizedBox(height: 3),
                          ElevatedButton(
                            onPressed: () async {
                              final res = await context.push(SeedScreen.route);

                              setState(() {
                                isUserSeedBackupConfirmed = Future.value(res as bool);
                              });
                            },
                            child: const Text("Backup Wallet"),
                          ),
                        ],
                      );
                    }
                    // return an empty box if the wallet has already been backed up or the data has not been fetched yet.
                    return const SizedBox(height: 0);
                  }),
              const SizedBox(
                height: 5,
              ),
              Expanded(
                child: ScrollConfiguration(
                  behavior: ScrollConfiguration.of(context).copyWith(
                    dragDevices: {
                      PointerDeviceKind.touch,
                      PointerDeviceKind.mouse,
                    },
                  ),
                  child: ListView.builder(
                    shrinkWrap: true,
                    physics: const AlwaysScrollableScrollPhysics(),
                    itemCount: walletChangeNotifier.walletInfo.history.length + 1,
                    itemBuilder: (BuildContext context, int index) {
                      // Spacer at the bottom of the list
                      if (index == walletChangeNotifier.walletInfo.history.length) {
                        return listBottomScrollSpace;
                      }

                      WalletHistoryItemData itemData =
                          walletChangeNotifier.walletInfo.history[index];

                      return itemData.toWidget();
                    },
                  ),
                ),
              ),
            ],
          ),
        ),
      ),
      floatingActionButton: SpeedDial(
        icon: SendReceiveIcons.sendReceive,
        iconTheme: const IconThemeData(size: 20),
        activeIcon: Icons.close,
        buttonSize: const Size(56.0, 56.0),
        visible: true,
        closeManually: false,
        curve: Curves.bounceIn,
        overlayColor: theme.dividerColor,
        overlayOpacity: 0.5,
        elevation: 8.0,
        shape: const CircleBorder(),
        children: [
          SpeedDialChild(
            child: const Icon(SendReceiveIcons.receive, size: 20.0),
            label: 'Receive',
            labelStyle: const TextStyle(fontSize: 18.0),
            // additionally checking the lightning balance here, as when hot reloading the app the channel info appears to be unknown.
            onTap: () => context.go((hasChannel || walletChangeNotifier.lightning().sats > 0)
                ? ReceiveScreen.route
                : OnboardingScreen.route),
          ),
          SpeedDialChild(
            child: const Icon(SendReceiveIcons.sendWithQr, size: 24.0),
            label: 'Send',
            labelStyle: const TextStyle(fontSize: 18.0),
            onTap: () => GoRouter.of(context).go(SendScreen.route),
          ),
        ],
      ),
    );
  }
}
