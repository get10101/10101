import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/common/settings/settings_screen.dart';
import 'package:get_10101/common/snack_bar.dart';
import 'package:get_10101/logger/logger.dart';

import 'package:go_router/go_router.dart';

import 'package:package_info_plus/package_info_plus.dart';
import 'package:get_10101/ffi.dart' as rust;

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
            child: SingleChildScrollView(
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
                              "App Info",
                              style: TextStyle(fontWeight: FontWeight.w500, fontSize: 20),
                            ),
                            // shift the row the size of the icon into the middle so that it is properly centered.
                            SizedBox(width: 24)
                          ],
                        ),
                      )
                    ],
                  ),
                  const SizedBox(
                    height: 20,
                  ),
                  Container(
                    margin: const EdgeInsets.all(10),
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
                              children: [moreInfo(context, title: "Node Id", info: _nodeId)],
                            ))
                      ],
                    ),
                  ),
                  const SizedBox(height: 20),
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
                                moreInfo(context,
                                    title: "Number", info: _buildNumber, showCopyButton: true),
                                moreInfo(context,
                                    title: "Version", info: _version, showCopyButton: true),
                                moreInfo(context, title: "Commit Hash", info: commit),
                                moreInfo(context,
                                    title: "Branch", info: branch, showCopyButton: true)
                              ],
                            ))
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
            !showCopyButton
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
        !showCopyButton
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
