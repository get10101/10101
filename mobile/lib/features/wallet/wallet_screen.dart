import 'package:flutter/material.dart';
import 'package:flutter_speed_dial/flutter_speed_dial.dart';
import 'package:get_10101/common/channel_status_notifier.dart';
import 'package:get_10101/features/wallet/balance.dart';
import 'package:get_10101/features/wallet/receive_screen.dart';
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

    WalletTheme theme = Theme.of(context).extension<WalletTheme>()!;

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
              if (walletChangeNotifier.lightning().sats == 0)
                Container(
                  margin: const EdgeInsets.only(left: 4, right: 4),
                  child: ElevatedButton(
                    onPressed: () {
                      context.go(OnboardingScreen.route);
                    },
                    child: const Text("Fund Wallet"),
                  ),
                ),
              FutureBuilder(
                  future: isUserSeedBackupConfirmed,
                  builder: (BuildContext context, AsyncSnapshot<bool> snapshot) {
                    // TODO(holzeis): Move backup seed phrase to settings
                    // FIXME: We ignore the value of `isUserSeedBackupConfirmed` stored in
                    // `snapshot.data` to keep the `Backup Wallet` button visible at all times for
                    // now. We need to rework this.
                    if (snapshot.connectionState == ConnectionState.done) {
                      return Container(
                        margin: const EdgeInsets.only(left: 4, right: 4),
                        child: Column(
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
                        ),
                      );
                    }
                    // return an empty box if the wallet has already been backed up or the data has not been fetched yet.
                    return const SizedBox(height: 0);
                  }),
              const SizedBox(
                height: 5,
              ),
              Expanded(
                child: SingleChildScrollView(
                  physics: const AlwaysScrollableScrollPhysics(),
                  child: Card(
                    child: Column(
                      children:
                          walletChangeNotifier.walletInfo.history.map((e) => e.toWidget()).toList(),
                    ),
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
