import 'package:flutter/material.dart';
import 'package:get_10101/common/dlc_channel_change_notifier.dart';
import 'package:get_10101/common/settings/settings_screen.dart';
import 'package:get_10101/common/snack_bar.dart';
import 'package:get_10101/features/wallet/wallet_screen.dart';
import 'package:go_router/go_router.dart';
import 'package:get_10101/ffi.dart' as rust;
import 'package:provider/provider.dart';
import 'package:slide_to_confirm/slide_to_confirm.dart';

class ForceCloseScreen extends StatefulWidget {
  static const route = "${SettingsScreen.route}/$subRouteName";
  static const subRouteName = "forceclose";

  const ForceCloseScreen({
    super.key,
  });

  @override
  State<ForceCloseScreen> createState() => _ForceCloseScreenState();
}

class _ForceCloseScreenState extends State<ForceCloseScreen> {
  bool isCloseChannelButtonDisabled = false;

  @override
  Widget build(BuildContext context) {
    DlcChannelChangeNotifier dlcChannelChangeNotifier = context.watch<DlcChannelChangeNotifier>();

    return Scaffold(
      body: Container(
          padding: const EdgeInsets.only(top: 20, left: 10, right: 10),
          child: SafeArea(
            child: Column(
              children: [
                Row(
                  mainAxisAlignment: MainAxisAlignment.center,
                  children: [
                    Expanded(
                      child: Stack(
                        children: [
                          GestureDetector(
                            child: Container(
                                alignment: AlignmentDirectional.topStart,
                                decoration: BoxDecoration(
                                    color: Colors.transparent,
                                    borderRadius: BorderRadius.circular(10)),
                                width: 70,
                                child: const Icon(
                                  Icons.arrow_back_ios_new_rounded,
                                  size: 22,
                                )),
                            onTap: () {
                              GoRouter.of(context).pop();
                            },
                          ),
                          const Row(
                            mainAxisAlignment: MainAxisAlignment.center,
                            children: [
                              Text(
                                "Force-Close Channel",
                                style: TextStyle(fontWeight: FontWeight.w500, fontSize: 20),
                              ),
                            ],
                          ),
                        ],
                      ),
                    ),
                  ],
                ),
                const SizedBox(
                  height: 20,
                ),
                Container(
                    margin: const EdgeInsets.all(10),
                    child: getForceCloseChannelText(dlcChannelChangeNotifier)),
                Expanded(child: Container()),
                Visibility(
                  visible: dlcChannelChangeNotifier.canForceClose(),
                  child: Container(
                    margin: const EdgeInsets.all(10),
                    child: ConfirmationSlider(
                        text: "Swipe to force-close",
                        textStyle: const TextStyle(color: Colors.black87, fontSize: 18),
                        height: 40,
                        foregroundColor: Colors.red,
                        sliderButtonContent: const Icon(
                          Icons.chevron_right,
                          color: Colors.white,
                          size: 20,
                        ),
                        onConfirmation: () async {
                          final messenger = ScaffoldMessenger.of(context);
                          try {
                            await rust.api
                                .forceCloseChannel()
                                .then((value) => GoRouter.of(context).go(WalletScreen.route));
                          } catch (e) {
                            showSnackBar(
                              messenger,
                              e.toString(),
                            );
                          }
                        }),
                  ),
                )
              ],
            ),
          )),
    );
  }
}

RichText getForceCloseChannelText(DlcChannelChangeNotifier dlcChannelChangeNotifier) {
  if (!dlcChannelChangeNotifier.hasDlcChannel()) {
    return RichText(
        text: const TextSpan(
      text:
          "You do not have a channel! Go fund your wallet and create one! Then you can come back here and force-close it.",
      style: TextStyle(fontSize: 18, color: Colors.black, letterSpacing: 0.4),
    ));
  }

  return RichText(
    text: const TextSpan(
      style: TextStyle(fontSize: 18, color: Colors.black, letterSpacing: 0.4),
      children: [
        TextSpan(
          text: "Warning",
          style: TextStyle(color: Colors.red, fontWeight: FontWeight.w600),
        ),
        TextSpan(
          text:
              ": Force-closing your channel should only be considered as a last resort if 10101 is not reachable.\n\n",
        ),
        TextSpan(
            text:
                "It's always better to collaboratively close as it will also save transaction fees.\n\n"),
        TextSpan(text: "If you "),
        TextSpan(
            text: "force-close", style: TextStyle(color: Colors.red, fontWeight: FontWeight.w600)),
        TextSpan(text: ", you will have to pay the fees for going on-chain.\n\n"),
        TextSpan(text: "Your funds can be claimed by your on-chain wallet after a while.\n\n"),
      ],
    ),
  );
}
