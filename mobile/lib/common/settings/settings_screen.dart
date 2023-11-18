import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart';
import 'package:get_10101/common/channel_status_notifier.dart';
import 'package:get_10101/common/color.dart';

import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/common/service_status_notifier.dart';
import 'package:get_10101/common/settings/app_info_screen.dart';
import 'package:get_10101/common/settings/channel_screen.dart';
import 'package:get_10101/common/settings/collab_close_screen.dart';
import 'package:get_10101/common/settings/force_close_screen.dart';
import 'package:get_10101/common/settings/share_logs_screen.dart';
import 'package:get_10101/common/snack_bar.dart';
import 'package:get_10101/common/status_screen.dart';

import 'package:get_10101/util/custom_icon_icons.dart';
import 'package:go_router/go_router.dart';

import 'package:provider/provider.dart';

class SettingsScreen extends StatefulWidget {
  static const route = "/settings";

  final String location;

  const SettingsScreen({super.key, required this.location});

  @override
  State<SettingsScreen> createState() => _SettingsScreenState();
}

class _SettingsScreenState extends State<SettingsScreen> {
  @override
  Widget build(BuildContext context) {
    final bridge.Config config = context.read<bridge.Config>();

    ChannelStatusNotifier channelStatusNotifier = context.watch<ChannelStatusNotifier>();
    ServiceStatusNotifier serviceStatusNotifier = context.watch<ServiceStatusNotifier>();

    final overallStatus = serviceStatusNotifier.overall();

    EdgeInsets margin = const EdgeInsets.all(10);
    return Scaffold(
      body: SafeArea(
          child: SingleChildScrollView(
        child: Container(
          padding: const EdgeInsets.only(top: 20, left: 10, right: 0),
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
                            GoRouter.of(context).go(widget.location);
                          },
                        ),
                        const Row(
                          mainAxisAlignment: MainAxisAlignment.center,
                          children: [
                            Text(
                              "Settings",
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
                margin: margin.copyWith(bottom: 20),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    const Text(
                      "GENERAL",
                      style: TextStyle(color: Colors.grey, fontSize: 17),
                    ),
                    const SizedBox(
                      height: 10,
                    ),
                    Container(
                      decoration: BoxDecoration(
                          color: Colors.white, borderRadius: BorderRadius.circular(10)),
                      child: Column(
                        children: [
                          SettingsClickable(
                              icon: Icons.info_outline,
                              title: "App Info",
                              callBackFunc: () => GoRouter.of(context).push(AppInfoScreen.route)),
                          SettingsClickable(
                              icon: Icons.feed_outlined,
                              title: "Share Logs",
                              callBackFunc: () => GoRouter.of(context).push(ShareLogsScreen.route)),
                          SettingsClickable(
                              icon: Icons.balance_outlined,
                              title: "Channel",
                              isAlarm: channelStatusNotifier.isClosing(),
                              callBackFunc: () => GoRouter.of(context).push(ChannelScreen.route)),
                        ],
                      ),
                    )
                  ],
                ),
              ),
              Container(
                margin: margin.copyWith(bottom: 20),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    const Text(
                      "ENDPOINT",
                      style: TextStyle(color: Colors.grey, fontSize: 17),
                    ),
                    const SizedBox(
                      height: 10,
                    ),
                    Container(
                      decoration: BoxDecoration(
                          color: Colors.white, borderRadius: BorderRadius.circular(10)),
                      child: Column(
                        children: [
                          SettingsClickable(
                            icon: CustomIcon.linkSolid,
                            title: "Esplora/Electrum",
                            info: config.esploraEndpoint,
                          ),
                          SettingsClickable(
                            icon: CustomIcon.tv,
                            title: "Coordinator",
                            info: "${config.coordinatorPubkey}@${config.host}:${config.p2PPort}",
                          ),
                          SettingsClickable(
                            icon: Icons.thermostat,
                            title: "Status",
                            isAlarm: overallStatus == ServiceStatus.Offline,
                            callBackFunc: () => GoRouter.of(context).push(StatusScreen.route),
                          ),
                        ],
                      ),
                    )
                  ],
                ),
              ),
              Container(
                margin: margin.copyWith(bottom: 20),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    const Text(
                      "DANGER ZONE",
                      style: TextStyle(color: Colors.grey, fontSize: 17),
                    ),
                    const SizedBox(
                      height: 10,
                    ),
                    Container(
                      decoration: BoxDecoration(
                          color: Colors.white, borderRadius: BorderRadius.circular(10)),
                      child: Column(
                        children: [
                          SettingsClickable(
                              icon: Icons.close,
                              title: "Close Channel",
                              callBackFunc: () =>
                                  GoRouter.of(context).push(CollabCloseScreen.route)),
                          Visibility(
                            visible: config.network == "regtest",
                            child: SettingsClickable(
                                icon: Icons.dangerous,
                                isAlarm: true,
                                title: "Force-Close Channel",
                                callBackFunc: () =>
                                    GoRouter.of(context).push(ForceCloseScreen.route)),
                          ),
                        ],
                      ),
                    ),
                  ],
                ),
              ),
            ],
          ),
        ),
      )),
    );
  }
}

class SettingsClickable extends StatefulWidget {
  const SettingsClickable({
    super.key,
    required this.icon,
    required this.title,
    this.callBackFunc,
    this.isAlarm = false,
    this.info,
  });

  final IconData icon;
  final String title;
  final void Function()? callBackFunc;
  final bool isAlarm;
  final String? info;

  @override
  State<SettingsClickable> createState() => _SettingsClickableState();
}

class _SettingsClickableState extends State<SettingsClickable> {
  bool isMoreInfo = false;

  @override
  Widget build(BuildContext context) {
    return GestureDetector(
      onTap: () {
        widget.callBackFunc != null
            ? widget.callBackFunc?.call()
            : setState(() => isMoreInfo = !isMoreInfo);
      },
      child: Container(
        decoration: BoxDecoration(border: Border.all(color: Colors.white, width: 0.0)),
        padding: const EdgeInsets.all(15),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          mainAxisAlignment: MainAxisAlignment.start,
          children: [
            Icon(
              widget.icon,
              size: 20,
              color: widget.isAlarm ? Colors.red.shade400 : tenTenOnePurple.shade800,
            ),
            const SizedBox(
              width: 20,
            ),
            Expanded(
              child: Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                mainAxisAlignment: MainAxisAlignment.spaceBetween,
                children: [
                  Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        widget.title,
                        style: TextStyle(
                            fontSize: 17,
                            fontWeight: FontWeight.w400,
                            color: widget.isAlarm ? Colors.red : Colors.black),
                      ),
                      isMoreInfo
                          ? Column(
                              children: [
                                const SizedBox(
                                  height: 10,
                                ),
                                SizedBox(
                                    width: MediaQuery.of(context).size.width - 130,
                                    child: Text(widget.info ?? ""))
                              ],
                            )
                          : const SizedBox()
                    ],
                  ),
                  isMoreInfo
                      ? GestureDetector(
                          onTap: () async {
                            showSnackBar(ScaffoldMessenger.of(context), "Copied ${widget.info}");
                            await Clipboard.setData(ClipboardData(text: widget.info ?? ""));
                          },
                          child: const Icon(
                            Icons.copy,
                            size: 17,
                            color: tenTenOnePurple,
                          ),
                        )
                      : const Icon(
                          Icons.arrow_forward_ios_rounded,
                          size: 17,
                          color: Colors.grey,
                        )
                ],
              ),
            )
          ],
        ),
      ),
    );
  }
}
