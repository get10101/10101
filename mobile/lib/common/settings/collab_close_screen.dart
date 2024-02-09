import 'package:flutter/material.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/common/dlc_channel_change_notifier.dart';
import 'package:get_10101/common/settings/settings_screen.dart';
import 'package:get_10101/common/snack_bar.dart';
import 'package:get_10101/features/trade/trade_screen.dart';
import 'package:get_10101/features/wallet/wallet_screen.dart';
import 'package:go_router/go_router.dart';
import 'package:get_10101/ffi.dart' as rust;
import 'package:provider/provider.dart';
import 'package:slide_to_confirm/slide_to_confirm.dart';

class CollabCloseScreen extends StatefulWidget {
  static const route = "${SettingsScreen.route}/$subRouteName";
  static const subRouteName = "collabclose";

  const CollabCloseScreen({
    super.key,
  });

  @override
  State<CollabCloseScreen> createState() => _CollabCloseScreenState();
}

class _CollabCloseScreenState extends State<CollabCloseScreen> {
  bool isCloseChannelButtonDisabled = false;

  @override
  void initState() {
    super.initState();
    context.read<DlcChannelChangeNotifier>().refreshDlcChannels();
  }

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
                                "Close Channel",
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
                    child: getCloseChannelText(dlcChannelChangeNotifier)),
                Expanded(child: Container()),
                Visibility(
                    visible: dlcChannelChangeNotifier.hasOpenPosition(),
                    child: Container(
                      margin: const EdgeInsets.all(10),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.stretch,
                        children: [
                          ElevatedButton(
                              onPressed: () => GoRouter.of(context).go(TradeScreen.route),
                              child: const Text("Go to trade")),
                        ],
                      ),
                    )),
                Visibility(
                  visible: dlcChannelChangeNotifier.hasDlcChannelWithoutPosition(),
                  child: Container(
                      margin: const EdgeInsets.all(10),
                      child: ConfirmationSlider(
                          text: "Swipe to collab-close",
                          textStyle: const TextStyle(color: Colors.black87, fontSize: 18),
                          height: 40,
                          foregroundColor: tenTenOnePurple,
                          sliderButtonContent: const Icon(
                            Icons.chevron_right,
                            color: Colors.white,
                            size: 20,
                          ),
                          onConfirmation: () async {
                            final messenger = ScaffoldMessenger.of(context);
                            rust.api
                                .closeChannel()
                                .then((value) => GoRouter.of(context).go(WalletScreen.route))
                                .catchError((e) {
                              showSnackBar(
                                messenger,
                                e.toString(),
                              );
                            });
                          })),
                )
              ],
            ),
          )),
    );
  }
}

RichText getCloseChannelText(DlcChannelChangeNotifier dlcChannelChangeNotifier) {
  if (dlcChannelChangeNotifier.hasOpenPosition()) {
    return RichText(
        text: const TextSpan(
      style: TextStyle(fontSize: 18, color: Colors.black, letterSpacing: 0.4),
      children: [
        TextSpan(text: "Please,"),
        TextSpan(
            text: " close your open position",
            style: TextStyle(color: tenTenOnePurple, fontWeight: FontWeight.w600)),
        TextSpan(text: " before collaboratively closing your channel"),
      ],
    ));
  }

  if (dlcChannelChangeNotifier.hasDlcChannel()) {
    return RichText(
        text: const TextSpan(
      style: TextStyle(fontSize: 18, color: Colors.black, letterSpacing: 0.4),
      children: [
        TextSpan(
          text:
              "By closing your channel you will receive your channel funds on-chain in the 10101 app.\n\n",
        ),
        TextSpan(text: "Use this feature if you want to "),
        TextSpan(
            text: "drain your funds",
            style: TextStyle(color: tenTenOnePurple, fontWeight: FontWeight.w600)),
        TextSpan(text: ", or to create a "),
        TextSpan(
            text: "bigger channel",
            style: TextStyle(color: tenTenOnePurple, fontWeight: FontWeight.w600)),
        TextSpan(text: " with 10101.")
      ],
    ));
  }

  return RichText(
      text: const TextSpan(
    text:
        "You do not have a channel! Go fund your wallet and create one! Then you can come back here and close it.",
    style: TextStyle(fontSize: 18, color: Colors.black, letterSpacing: 0.4),
  ));
}
