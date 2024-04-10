import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:font_awesome_flutter/font_awesome_flutter.dart';
import 'package:get_10101/common/poll_widget.dart';
import 'package:get_10101/common/secondary_action_button.dart';
import 'package:get_10101/features/wallet/balance.dart';
import 'package:get_10101/features/wallet/receive_screen.dart';
import 'package:get_10101/features/wallet/referral_widget.dart';
import 'package:get_10101/features/wallet/scanner_screen.dart';
import 'package:get_10101/features/wallet/wallet_change_notifier.dart';
import 'package:get_10101/util/poll_change_notified.dart';
import 'package:get_10101/util/preferences.dart';
import 'package:go_router/go_router.dart';
import 'package:flutter/scheduler.dart';
import 'package:provider/provider.dart';
import 'package:screenshot/screenshot.dart';
import 'package:get_10101/ffi.dart' as rust;

class WalletScreen extends StatefulWidget {
  static const route = "/wallet";
  static const label = "Wallet";

  const WalletScreen({Key? key}) : super(key: key);

  @override
  State<WalletScreen> createState() => _WalletScreenState();
}

class _WalletScreenState extends State<WalletScreen> {
  ScreenshotController screenShotController = ScreenshotController();

  @override
  void initState() {
    super.initState();

    if (rust.api.hasTradedOnce()) {
      SchedulerBinding.instance.addPostFrameCallback((_) {
        _afterLoaded();
      });
    }
  }

  _afterLoaded() async {
    final Preferences preferences = Preferences.instance;
    preferences.hasReferralDialogTimePassedMoreThan7days().then((timeToShowDialogAgain) {
      if (timeToShowDialogAgain) {
        showDialog(
          context: context,
          builder: (BuildContext context) {
            return ShareReferralWidget(
                screenShotController: screenShotController, pref: preferences);
          },
        );
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    final pollChangeNotifier = context.watch<PollChangeNotifier>();
    final walletChangeNotifier = context.watch<WalletChangeNotifier>();

    return Scaffold(
      body: RefreshIndicator(
        onRefresh: () async {
          await walletChangeNotifier.refreshWalletInfo();
          await walletChangeNotifier.waitForSyncToComplete();
          await pollChangeNotifier.refresh();
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
                      child: SecondaryActionButton(
                        onPressed: () => context.go(ReceiveScreen.route),
                        icon: FontAwesomeIcons.arrowDown,
                        title: 'Receive',
                      ),
                    ),
                    const SizedBox(width: 10.0),
                    Expanded(
                        child: SecondaryActionButton(
                      onPressed: () => GoRouter.of(context).go(ScannerScreen.route),
                      icon: FontAwesomeIcons.arrowUp,
                      title: 'Send',
                    ))
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
                    child: Column(
                      children: [
                        const Padding(
                          padding: EdgeInsets.only(bottom: 8.0),
                          child: PollWidget(),
                        ),
                        Card(
                          margin: const EdgeInsets.all(0.0),
                          elevation: 1,
                          child: Column(
                            children: walletChangeNotifier.walletInfo.history
                                .map((e) => e.toWidget())
                                .toList(),
                          ),
                        ),
                      ],
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
}
