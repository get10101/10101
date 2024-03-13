import 'dart:convert';
import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:get_10101/common/application/clickable_help_text.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/common/more_info.dart';
import 'package:get_10101/logger/hybrid_logger.dart';
import 'package:get_10101/logger/logger.dart';
import 'package:flutter/material.dart';
import 'package:intl/intl.dart';
import 'package:path_provider/path_provider.dart';
import 'package:share_plus/share_plus.dart';

class ErrorScreen extends StatefulWidget {
  static const route = "/error";
  static const label = "Error";

  const ErrorScreen({Key? key}) : super(key: key);

  @override
  State<ErrorScreen> createState() => _ErrorScreenState();
}

class _ErrorScreenState extends State<ErrorScreen> {
  @override
  Widget build(BuildContext context) {
    return AnnotatedRegion<SystemUiOverlayStyle>(
        value: SystemUiOverlayStyle.dark,
        child: Scaffold(
          body: SafeArea(
            child: Container(
              padding: const EdgeInsets.only(top: 20, left: 20, right: 20),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Container(
                    margin: const EdgeInsets.all(10),
                    child: Column(
                      children: [
                        const SizedBox(height: 20),
                        const Text(
                          "Failed to start 10101!",
                          style: TextStyle(fontSize: 22, fontWeight: FontWeight.w400),
                        ),
                        const SizedBox(height: 40),
                        const Icon(Icons.error_outline_rounded, color: Colors.red, size: 100),
                        const SizedBox(height: 40),
                        const ClickableHelpText(
                            text: "Please help us fix this issue and join our telegram group: ",
                            style: TextStyle(fontSize: 17, color: Colors.black87)),
                        const SizedBox(height: 30),
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
                                          const SizedBox(width: 20),
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
                  )
                ],
              ),
            ),
          ),
        ));
  }
}
