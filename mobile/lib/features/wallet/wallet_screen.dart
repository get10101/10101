import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:font_awesome_flutter/font_awesome_flutter.dart';
import 'package:get_10101/common/channel_status_notifier.dart';
import 'package:get_10101/features/swap/swap_screen.dart';
import 'package:get_10101/features/wallet/balance.dart';
import 'package:get_10101/features/wallet/receive_screen.dart';
import 'package:get_10101/features/wallet/onboarding/onboarding_screen.dart';
import 'package:get_10101/features/wallet/scanner_screen.dart';
import 'package:get_10101/features/wallet/wallet_change_notifier.dart';
import 'package:go_router/go_router.dart';
import 'package:provider/provider.dart';

class WalletScreen extends StatelessWidget {
  static const route = "/wallet";
  static const label = "Wallet";

  const WalletScreen({Key? key}) : super(key: key);

  @override
  Widget build(BuildContext context) {
    final walletChangeNotifier = context.watch<WalletChangeNotifier>();

    final hasChannel = context.watch<ChannelStatusNotifier>().hasChannel();

    ButtonStyle balanceButtonStyle = balanceActionButtonStyle();

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
              const Balance(),
              const SizedBox(height: 10.0),
              Container(
                  margin: const EdgeInsets.only(left: 0, right: 0),
                  child: Row(children: [
                    Expanded(
                      child: ElevatedButton(
                        style: balanceButtonStyle,
                        // Additionally checking the Lightning balance here, as when
                        // hot reloading the app the channel info appears to be
                        // unknown.
                        onPressed: () => context.go(
                            (hasChannel || walletChangeNotifier.lightning().sats > 0)
                                ? ReceiveScreen.route
                                : OnboardingScreen.route),
                        child: const Row(
                          mainAxisAlignment: MainAxisAlignment.center,
                          children: [
                            Icon(
                              FontAwesomeIcons.arrowDown,
                              size: 14,
                            ),
                            SizedBox(width: 7, height: 40),
                            Text(
                              'Receive',
                              style: TextStyle(fontSize: 14, fontWeight: FontWeight.normal),
                            )
                          ],
                        ),
                      ),
                    ),
                    const SizedBox(width: 10.0),
                    Expanded(
                      child: ElevatedButton(
                        style: balanceButtonStyle,
                        onPressed: () => showSwapDrawer(context),
                        child: const Row(
                          mainAxisAlignment: MainAxisAlignment.center,
                          children: [
                            Icon(
                              FontAwesomeIcons.rotate,
                              size: 14,
                            ),
                            SizedBox(
                              width: 7,
                              height: 40,
                            ),
                            Text(
                              'Swap',
                              style: TextStyle(fontSize: 14, fontWeight: FontWeight.normal),
                            )
                          ],
                        ),
                      ),
                    ),
                    const SizedBox(width: 10.0),
                    Expanded(
                      child: ElevatedButton(
                        style: balanceButtonStyle,
                        onPressed: () => GoRouter.of(context).go(ScannerScreen.route),
                        child: const Row(
                          mainAxisAlignment: MainAxisAlignment.center,
                          children: [
                            Icon(
                              FontAwesomeIcons.arrowUp,
                              size: 14,
                            ),
                            SizedBox(
                              width: 7,
                              height: 40,
                            ),
                            Text(
                              'Send',
                              style: TextStyle(fontSize: 14, fontWeight: FontWeight.normal),
                            )
                          ],
                        ),
                      ),
                    )
                  ])),
              const SizedBox(
                height: 10,
              ),
              Expanded(
                child: ScrollConfiguration(
                  behavior: ScrollConfiguration.of(context).copyWith(
                    dragDevices: PointerDeviceKind.values.toSet(),
                  ),
                  child: SingleChildScrollView(
                    physics: const AlwaysScrollableScrollPhysics(),
                    child: Card(
                      margin: const EdgeInsets.all(0.0),
                      elevation: 1,
                      child: Column(
                        children: walletChangeNotifier.walletInfo.history
                            .map((e) => e.toWidget())
                            .toList(),
                      ),
                    ),
                  ),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }

  ButtonStyle balanceActionButtonStyle() {
    ColorScheme greyScheme = ColorScheme.fromSwatch(primarySwatch: Colors.grey);

    return IconButton.styleFrom(
      foregroundColor: Colors.black,
      backgroundColor: Colors.grey.shade200,
      disabledBackgroundColor: greyScheme.onSurface.withOpacity(0.12),
      elevation: 0,
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(10.0)),
      hoverColor: greyScheme.onPrimary.withOpacity(0.08),
      focusColor: greyScheme.onPrimary.withOpacity(0.12),
      highlightColor: greyScheme.onPrimary.withOpacity(0.12),
      visualDensity: const VisualDensity(horizontal: 0.0, vertical: 1.0),
    );
  }
}
