import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:font_awesome_flutter/font_awesome_flutter.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/common/settings/open_telegram.dart';
import 'package:get_10101/common/settings/settings_screen.dart';
import 'package:get_10101/common/snack_bar.dart';
import 'package:get_10101/logger/logger.dart';

import 'package:go_router/go_router.dart';

import 'package:package_info_plus/package_info_plus.dart';
import 'package:get_10101/ffi.dart' as rust;
import 'package:url_launcher/url_launcher.dart';

class AppInfoScreen extends StatefulWidget {
  static const route = "${SettingsScreen.route}/$subRouteName";
  static const subRouteName = "appdetails";

  const AppInfoScreen({super.key});

  @override
  State<AppInfoScreen> createState() => _AppInfoScreenState();
}

class _AppInfoScreenState extends State<AppInfoScreen> {
  EdgeInsets margin = const EdgeInsets.all(10);

  String _buildNumber = '';
  String _version = '';
  String _nodeId = "";

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

    setState(() {
      _buildNumber = value.buildNumber;
      _version = value.version;
    });
  }

  @override
  Widget build(BuildContext context) {
    String commit = const String.fromEnvironment('COMMIT', defaultValue: 'not available');
    String branch = const String.fromEnvironment('BRANCH', defaultValue: 'not available');

    return Scaffold(
      body: Container(
          padding: const EdgeInsets.only(top: 20, left: 10, right: 10),
          child: SafeArea(
            child: Column(
              children: [
                SingleChildScrollView(
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
                                    "App Info",
                                    style: TextStyle(fontWeight: FontWeight.w500, fontSize: 20),
                                  ),
                                ],
                              ),
                            ],
                          ),
                        ),
                      ],
                    ),
                    Container(
                      margin: const EdgeInsets.only(top: 20, left: 10, right: 10, bottom: 10),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          const Text(
                            "NODE INFO",
                            style: TextStyle(color: Colors.grey, fontSize: 17),
                          ),
                          const SizedBox(
                            height: 10,
                          ),
                          Container(
                              decoration: BoxDecoration(
                                  color: Colors.white, borderRadius: BorderRadius.circular(15)),
                              child: Column(
                                children: [
                                  moreInfo(context,
                                      title: "Node Id", info: _nodeId, showCopyButton: true)
                                ],
                              ))
                        ],
                      ),
                    ),
                    Container(
                      margin: const EdgeInsets.all(10),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          const Text(
                            "BUILD INFO",
                            style: TextStyle(color: Colors.grey, fontSize: 18),
                          ),
                          const SizedBox(
                            height: 10,
                          ),
                          Container(
                              decoration: BoxDecoration(
                                  color: Colors.white, borderRadius: BorderRadius.circular(15)),
                              child: Column(
                                children: [
                                  moreInfo(context, title: "Number", info: _buildNumber),
                                  moreInfo(context, title: "Version", info: _version),
                                  moreInfo(context,
                                      title: "Commit Hash", info: commit, showCopyButton: true),
                                  moreInfo(context,
                                      title: "Branch", info: branch, showCopyButton: kDebugMode)
                                ],
                              ))
                        ],
                      ),
                    ),
                  ],
                )),
                Expanded(
                  child: Row(
                    mainAxisAlignment: MainAxisAlignment.spaceEvenly,
                    crossAxisAlignment: CrossAxisAlignment.end,
                    children: [
                      IconButton(
                        icon: const Icon(FontAwesomeIcons.twitter),
                        iconSize: 22,
                        onPressed: () async {
                          final messenger = ScaffoldMessenger.of(context);
                          final httpsUri = Uri(scheme: "https", host: "x.com", path: "get10101");
                          if (await canLaunchUrl(httpsUri)) {
                            await launchUrl(httpsUri, mode: LaunchMode.externalApplication);
                          } else {
                            showSnackBar(messenger, "Failed to open link");
                          }
                        },
                      ),
                      IconButton(
                        icon: const Icon(FontAwesomeIcons.github, size: 22),
                        onPressed: () async {
                          await openTelegram(context);
                        },
                      ),
                      IconButton(
                        icon: const Icon(FontAwesomeIcons.telegram, size: 22),
                        onPressed: () async {
                          final messenger = ScaffoldMessenger.of(context);
                          final httpsUri = Uri(scheme: "https", host: "t.me", path: "get10101");
                          if (await canLaunchUrl(httpsUri)) {
                            await launchUrl(httpsUri, mode: LaunchMode.externalApplication);
                          } else {
                            showSnackBar(messenger, "Failed to open link");
                          }
                        },
                      ),
                      IconButton(
                        icon: const Icon(FontAwesomeIcons.earthEurope, size: 22),
                        onPressed: () async {
                          final messenger = ScaffoldMessenger.of(context);
                          final httpsUri =
                              Uri(scheme: "https", host: "10101.finance", path: "blog");
                          if (await canLaunchUrl(httpsUri)) {
                            await launchUrl(httpsUri, mode: LaunchMode.externalApplication);
                          } else {
                            showSnackBar(messenger, "Failed to open link");
                          }
                        },
                      ),
                      IconButton(
                        icon: const Icon(FontAwesomeIcons.question, size: 22),
                        onPressed: () async {
                          final messenger = ScaffoldMessenger.of(context);
                          final httpsUri =
                              Uri(scheme: "https", host: "10101.finance", path: "blog/faq");
                          if (await canLaunchUrl(httpsUri)) {
                            await launchUrl(httpsUri, mode: LaunchMode.externalApplication);
                          } else {
                            showSnackBar(messenger, "Failed to open link");
                          }
                        },
                      )
                    ],
                  ),
                ),
                const SizedBox(height: 10)
              ],
            ),
          )),
    );
  }
}

Widget moreInfo(BuildContext context,
    {required String title, required String info, bool showCopyButton = false}) {
  return Container(
    padding: const EdgeInsets.all(15),
    child: Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisAlignment: MainAxisAlignment.spaceBetween,
      children: [
        Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              title,
              style:
                  const TextStyle(fontSize: 17, fontWeight: FontWeight.w400, color: Colors.black),
            ),
            const SizedBox(
              height: 7,
            ),
            showCopyButton
                ? SizedBox(
                    width: MediaQuery.of(context).size.width - 100,
                    child: Text(
                      info,
                      style: TextStyle(
                          fontSize: 18, fontWeight: FontWeight.w300, color: Colors.grey.shade700),
                    ))
                : const SizedBox()
          ],
        ),
        showCopyButton
            ? GestureDetector(
                onTap: () async {
                  showSnackBar(ScaffoldMessenger.of(context), "Copied $info");
                  await Clipboard.setData(ClipboardData(text: info));
                },
                child: Icon(
                  Icons.copy,
                  size: 17,
                  color: tenTenOnePurple.shade800,
                ),
              )
            : Text(
                info,
                style: TextStyle(
                    fontSize: 18, fontWeight: FontWeight.w300, color: Colors.grey.shade700),
              )
      ],
    ),
  );
}
