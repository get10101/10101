import 'dart:convert';
import 'dart:io';

import 'package:flutter/foundation.dart' show defaultTargetPlatform, TargetPlatform;
import 'package:flutter/material.dart';
import 'package:get_10101/common/application/switch.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/common/more_info.dart';
import 'package:get_10101/common/settings/settings_screen.dart';
import 'package:get_10101/logger/hybrid_logger.dart';
import 'package:get_10101/logger/logger.dart';
import 'package:get_10101/util/preferences.dart';
import 'package:go_router/go_router.dart';
import 'package:intl/intl.dart';
import 'package:path_provider/path_provider.dart';
import 'package:share_plus/share_plus.dart';

class ShareLogsScreen extends StatefulWidget {
  static const route = "${SettingsScreen.route}/$subRouteName";
  static const subRouteName = "sharelogs";

  const ShareLogsScreen({super.key});

  @override
  State<ShareLogsScreen> createState() => _ShareLogsScreenState();
}

class _ShareLogsScreenState extends State<ShareLogsScreen> {
  bool changedLogLevel = false;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: SafeArea(
        child: Container(
          padding: const EdgeInsets.only(top: 20, left: 10, right: 10),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
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
                              "Logs",
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
                margin: const EdgeInsets.all(10),
                child: Column(
                  children: [
                    const SizedBox(
                      height: 20,
                    ),
                    const Text(
                      "You can  either  export your application logs or save them for future reference.",
                      style: TextStyle(fontSize: 18, fontWeight: FontWeight.w400),
                    ),
                    const SizedBox(
                      height: 20,
                    ),
                    Row(
                      mainAxisAlignment: MainAxisAlignment.spaceBetween,
                      children: [
                        FutureBuilder(
                            future: HybridOutput.logFilePath(),
                            builder: (BuildContext context, AsyncSnapshot<File> snapshot) {
                              switch (defaultTargetPlatform) {
                                case TargetPlatform.android:
                                case TargetPlatform.iOS:
                                  return GestureDetector(
                                    onTap: () async {
                                      logger.toString();

                                      var logsAsString = await snapshot.data!.readAsString();
                                      final List<int> bytes = utf8.encode(logsAsString);
                                      final Directory tempDir = await getTemporaryDirectory();
                                      String now =
                                          DateFormat('yyyy-MM-dd_HHmmss').format(DateTime.now());
                                      final String filePath = '${tempDir.path}/$now.log';
                                      await File(filePath).writeAsBytes(bytes);
                                      final XFile logFile = XFile(filePath);
                                      Share.shareXFiles([logFile], text: 'Logs from $now');
                                    },
                                    child: Container(
                                      padding:
                                          const EdgeInsets.symmetric(vertical: 10, horizontal: 20),
                                      decoration: BoxDecoration(
                                          color: Colors.white,
                                          borderRadius: BorderRadius.circular(15)),
                                      child: Row(
                                        children: [
                                          Icon(
                                            Icons.ios_share_outlined,
                                            color: tenTenOnePurple.shade800,
                                            size: 22,
                                          ),
                                          const SizedBox(
                                            width: 20,
                                          ),
                                          Text(
                                            "Share Logs",
                                            style: TextStyle(
                                                color: tenTenOnePurple.shade800,
                                                fontSize: 16,
                                                fontWeight: FontWeight.w500),
                                          )
                                        ],
                                      ),
                                    ),
                                  );
                                default:
                                  return moreInfo(context,
                                      title: "Log file location",
                                      info: snapshot.data!.path,
                                      showCopyButton: true);
                              }
                            }),
                      ],
                    ),
                    const SizedBox(height: 20),
                    Container(
                      padding: const EdgeInsets.symmetric(vertical: 10, horizontal: 20),
                      decoration: BoxDecoration(
                          color: Colors.white, borderRadius: BorderRadius.circular(15)),
                      child: Row(
                        mainAxisAlignment: MainAxisAlignment.spaceBetween,
                        children: [
                          Text(
                            "Set log level to trace",
                            style: TextStyle(
                                color: tenTenOnePurple.shade800,
                                fontSize: 16,
                                fontWeight: FontWeight.w500),
                          ),
                          FutureBuilder(
                              future: Preferences.instance.isLogLevelTrace(),
                              builder: (BuildContext context, AsyncSnapshot<bool> snapshot) {
                                if (!snapshot.hasData) {
                                  return Container();
                                }

                                logger.i("trace: ${snapshot.data}");
                                return TenTenOneSwitch(
                                    value: snapshot.data ?? false,
                                    onChanged: (value) {
                                      setState(() {
                                        Preferences.instance.setLogLevelTrace(value);
                                        changedLogLevel = true;
                                      });
                                    });
                              }),
                        ],
                      ),
                    ),
                    const SizedBox(height: 20),
                    Visibility(
                      visible: changedLogLevel,
                      child: Container(
                          padding: const EdgeInsets.symmetric(vertical: 10, horizontal: 20),
                          decoration: BoxDecoration(
                              border: Border.all(color: tenTenOnePurple),
                              color: Colors.white,
                              borderRadius: BorderRadius.circular(10)),
                          child: Row(
                            mainAxisAlignment: MainAxisAlignment.start,
                            children: [
                              Icon(
                                Icons.info_outline,
                                color: tenTenOnePurple.shade800,
                                size: 22,
                              ),
                              const SizedBox(width: 10),
                              const Text("Restart the app to apply your changes."),
                            ],
                          )),
                    )
                  ],
                ),
              )
            ],
          ),
        ),
      ),
    );
  }
}
