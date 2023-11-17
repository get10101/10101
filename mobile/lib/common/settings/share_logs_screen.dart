import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/common/settings/settings_screen.dart';
import 'package:get_10101/logger/hybrid_logger.dart';
import 'package:get_10101/logger/logger.dart';
import 'package:go_router/go_router.dart';
import 'package:intl/intl.dart';
import 'package:path_provider/path_provider.dart';
import 'package:share_plus/share_plus.dart';

class ShareLogsScreen extends StatelessWidget {
  static const route = "${SettingsScreen.route}/$subRouteName";
  static const subRouteName = "sharelogs";

  const ShareLogsScreen({super.key});

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
                    GestureDetector(
                      onTap: () async {
                        logger.toString();

                        var file = await HybridOutput.logFilePath();
                        var logsAsString = await file.readAsString();
                        final List<int> bytes = utf8.encode(logsAsString);
                        final Directory tempDir = await getTemporaryDirectory();
                        String now = DateFormat('yyyy-MM-dd_HHmmss').format(DateTime.now());
                        final String filePath = '${tempDir.path}/$now.log';
                        await File(filePath).writeAsBytes(bytes);
                        final XFile logFile = XFile(filePath);
                        Share.shareXFiles([logFile], text: 'Logs from $now');
                      },
                      child: Container(
                        padding: const EdgeInsets.symmetric(vertical: 10, horizontal: 20),
                        decoration: BoxDecoration(
                            color: Colors.white, borderRadius: BorderRadius.circular(15)),
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
