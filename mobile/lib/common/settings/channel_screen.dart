import 'package:flutter/material.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart';
import 'package:get_10101/common/channel_status_notifier.dart';
import 'package:get_10101/common/settings/settings_screen.dart';
import 'package:get_10101/common/value_data_row.dart';
import 'package:go_router/go_router.dart';
import 'package:provider/provider.dart';

class ChannelScreen extends StatefulWidget {
  static const route = "${SettingsScreen.route}/$subRouteName";
  static const subRouteName = "channel";

  const ChannelScreen({
    super.key,
  });

  @override
  State<ChannelScreen> createState() => _ChannelScreenState();
}

class _ChannelScreenState extends State<ChannelScreen> {
  bool isCloseChannelButtonDisabled = false;

  @override
  Widget build(BuildContext context) {
    ChannelStatusNotifier channelStatusNotifier = context.watch<ChannelStatusNotifier>();

    final channelStatus = channelStatusToString(channelStatusNotifier.getChannelStatus());

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
                                "Channel",
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
                  height: 10,
                ),
                Padding(
                    padding: const EdgeInsets.all(10.0),
                    child: Column(
                      children: [
                        const SizedBox(height: 20),
                        ValueDataRow(
                          type: ValueType.text,
                          value: channelStatus,
                          label: "Channel status",
                          labelTextStyle: const TextStyle(fontSize: 18),
                          valueTextStyle:
                              const TextStyle(fontWeight: FontWeight.bold, fontSize: 18),
                        ),
                      ],
                    )),
                Visibility(
                    visible: channelStatusNotifier.isClosing(),
                    child: Padding(
                        padding: const EdgeInsets.only(top: 20, left: 10, right: 10),
                        child: RichText(
                            text: const TextSpan(
                                style: TextStyle(color: Colors.black, fontSize: 18),
                                children: [
                              TextSpan(
                                  text: "Your channel with 10101 is being closed on-chain!\n\n",
                                  style: TextStyle(fontWeight: FontWeight.bold)),
                              TextSpan(
                                  text:
                                      "Your off-chain funds will return back to your on-chain wallet after some time.\n\n"),
                              TextSpan(
                                  text:
                                      "If you had a position open your payout will arrive in your on-chain wallet soon after the expiry time. \n")
                            ]))))
              ],
            ),
          )),
    );
  }
}

String channelStatusToString(ChannelStatus status) {
  switch (status) {
    case ChannelStatus.NotOpen:
      return "Not open";
    case ChannelStatus.WithPosition:
      return "With Position";
    case ChannelStatus.Renewing:
    case ChannelStatus.Settling:
      return "Pending";
    case ChannelStatus.Closing:
      return "Closing";
    case ChannelStatus.Unknown:
      return "Unknown";
    case ChannelStatus.Open:
      return "Open";
  }
}
