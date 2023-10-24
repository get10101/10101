import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:get_10101/common/channel_status_notifier.dart';
import 'package:get_10101/common/color.dart';

import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/common/settings_details.dart';
import 'package:get_10101/common/settings_force_close.dart';
import 'package:get_10101/common/settings_logs.dart';
import 'package:get_10101/common/snack_bar.dart';
import 'package:get_10101/features/trade/position_change_notifier.dart';

import 'package:get_10101/util/custom_icon_icons.dart';
import 'package:go_router/go_router.dart';

import 'package:provider/provider.dart';

import 'package:package_info_plus/package_info_plus.dart';
import 'package:get_10101/ffi.dart' as rust;
import 'package:get_10101/logger/logger.dart';

class SettingsScreen extends StatefulWidget {
  const SettingsScreen({super.key});

  @override
  State<SettingsScreen> createState() => _SettingsScreenState();
}

class _SettingsScreenState extends State<SettingsScreen> {
  String _buildNumber = '';
  String _version = '';
  String _nodeId = "";

  // Variable preventing the user from spamming the close channel buttons
  bool _isCloseChannelButtonDisabled = false;

  @override
  void initState() {
    try {
      var nodeId = rust.api.getNodeId();
      _nodeId = nodeId;
    } catch (e) {
      logger.e("Error getting node id: $e");
      _nodeId = "UNKNOWN";
    }

    loadValues();
    super.initState();
  }

  Future<void> loadValues() async {
    var value = await PackageInfo.fromPlatform();

    logger.i("All values $value");
    setState(() {
      _buildNumber = value.buildNumber;
      _version = value.version;
    });
  }

  @override
  Widget build(BuildContext context) {
    final bridge.Config config = context.read<bridge.Config>();

    String commit = const String.fromEnvironment('COMMIT', defaultValue: 'not available');
    String branch = const String.fromEnvironment('BRANCH', defaultValue: 'not available');
    EdgeInsets margin = const EdgeInsets.all(10);
    return Scaffold(
      body: SafeArea(
          child: SingleChildScrollView(
        child: Container(
          padding: const EdgeInsets.all(10),
          child: Column(
            children: [
              Row(
                mainAxisAlignment: MainAxisAlignment.center,
                children: [
                  Row(
                    children: [
                      GestureDetector(
                        child: const Icon(
                          Icons.arrow_back_ios_new_rounded,
                          size: 22,
                        ),
                        onTap: () {
                          context.pop();
                        },
                      ),
                    ],
                  ),
                  const Expanded(
                    child: Row(
                      mainAxisAlignment: MainAxisAlignment.center,
                      children: [
                        Text(
                          "Settings",
                          style: TextStyle(fontWeight: FontWeight.w500, fontSize: 20),
                        ),
                        // shift the row the size of the icon into the middle so that it is properly centered.
                        SizedBox(width: 22)
                      ],
                    ),
                  )
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
                    Text(
                      "General".toUpperCase(),
                      style: const TextStyle(color: Colors.grey, fontSize: 17),
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
                              callBackFunc: (() {
                                Navigator.of(context).push(MaterialPageRoute(
                                  builder: (context) {
                                    return AppDetails(
                                      nodeId: _nodeId,
                                      version: _version,
                                      number: _buildNumber,
                                      branch: branch,
                                      commitHash: commit,
                                    );
                                  },
                                ));
                              })),
                          SettingsClickable(
                              icon: Icons.feed_outlined,
                              title: "Share Logs",
                              callBackFunc: (() {
                                Navigator.of(context).push(MaterialPageRoute(
                                  builder: (context) {
                                    return const ShareLogs();
                                  },
                                ));
                              })),
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
                    Text(
                      "Endpoint".toUpperCase(),
                      style: const TextStyle(color: Colors.grey, fontSize: 17),
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
                    Text(
                      "Danger Zone".toUpperCase(),
                      style: const TextStyle(color: Colors.grey, fontSize: 17),
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
                            callBackFunc: _isCloseChannelButtonDisabled
                                ? null
                                : () async {
                                    setState(() {
                                      _isCloseChannelButtonDisabled = true;
                                    });
                                    final messenger = ScaffoldMessenger.of(context);
                                    try {
                                      ensureCanCloseChannel(context);
                                      await rust.api.closeChannel();
                                    } catch (e) {
                                      showSnackBar(messenger, e.toString());
                                    } finally {
                                      setState(() {
                                        _isCloseChannelButtonDisabled = false;
                                      });
                                    }
                                  },
                          ),
                          Visibility(
                            visible: config.network == "regtest",
                            child: SettingsClickable(
                                icon: Icons.dangerous,
                                isForce: true,
                                title: "Force-Close Channel",
                                callBackFunc: (() {
                                  Navigator.of(context).push(MaterialPageRoute(
                                    builder: (context) {
                                      return const ForceClose();
                                    },
                                  ));
                                })),
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

/// Throws if the channel is not in a state where it can be closed.
void ensureCanCloseChannel(BuildContext context) {
  if (context.read<PositionChangeNotifier>().positions.isNotEmpty) {
    throw Exception("In order to close your Lighting Channel you need to close all your positions");
  }
  if (context.read<ChannelStatusNotifier>().isClosing()) {
    throw Exception("Your channel is already closing");
  }
}

class SettingsClickable extends StatefulWidget {
  const SettingsClickable({
    super.key,
    required this.icon,
    required this.title,
    this.callBackFunc,
    this.isForce = false,
    this.info,
  });

  final IconData icon;
  final String title;
  final void Function()? callBackFunc;
  final bool isForce;
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
        padding: const EdgeInsets.all(15),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          mainAxisAlignment: MainAxisAlignment.start,
          children: [
            Icon(
              widget.icon,
              size: 22,
              color: widget.isForce ? Colors.red.shade400 : tenTenOnePurple.shade800,
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
                            color: widget.isForce ? Colors.red : Colors.black),
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
