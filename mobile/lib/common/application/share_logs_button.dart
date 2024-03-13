import 'dart:convert';
import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/common/more_info.dart';
import 'package:get_10101/logger/hybrid_logger.dart';
import 'package:get_10101/logger/logger.dart';
import 'package:intl/intl.dart';
import 'package:path_provider/path_provider.dart';
import 'package:share_plus/share_plus.dart';

class ShareLogsButton extends StatelessWidget {
  const ShareLogsButton({super.key});

  @override
  Widget build(BuildContext context) {
    return FutureBuilder(
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
                  String now = DateFormat('yyyy-MM-dd_HHmmss').format(DateTime.now());
                  final String filePath = '${tempDir.path}/$now.log';
                  await File(filePath).writeAsBytes(bytes);
                  final XFile logFile = XFile(filePath);
                  Share.shareXFiles([logFile], text: 'Logs from $now');
                },
                child: Container(
                  padding: const EdgeInsets.symmetric(vertical: 10, horizontal: 20),
                  decoration:
                      BoxDecoration(color: Colors.white, borderRadius: BorderRadius.circular(15)),
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
                  title: "Log file location", info: snapshot.data!.path, showCopyButton: true);
          }
        });
  }
}
