import 'dart:convert';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:flutter/material.dart';
import 'package:get_10101/common/global_keys.dart';
import 'package:get_10101/logger/logger.dart';
import 'package:get_10101/util/coordinator_version.dart';
import 'package:http/http.dart' as http;
import 'package:package_info_plus/package_info_plus.dart';
import 'package:version/version.dart';

/// Compare the version of the coordinator with the version of the app
///
/// - If the coordinator is newer, suggest to update the app.
/// - If the app is newer, log it.
/// - If the coordinator cannot be reached, show a warning that the app may not function properly.
Future<void> compareCoordinatorVersion(bridge.Config config) async {
  PackageInfo packageInfo = await PackageInfo.fromPlatform();
  try {
    final response = await http.get(
      Uri.parse('http://${config.host}:${config.httpPort}/api/version'),
    );

    final clientVersion = Version.parse(packageInfo.version);
    final coordinatorVersion = CoordinatorVersion.fromJson(jsonDecode(response.body));
    logger.i("Coordinator version: ${coordinatorVersion.version.toString()}");

    if (coordinatorVersion.version > clientVersion) {
      logger.w("Client out of date. Current version: ${clientVersion.toString()}");
      showDialog(
          context: shellNavigatorKey.currentContext!,
          builder: (context) => AlertDialog(
                  title: const Text("Update available"),
                  content: Text("A new version of 10101 is available: "
                      "${coordinatorVersion.version.toString()}.\n\n"
                      "Please note that if you do not update 10101, the app"
                      " may not function properly."),
                  actions: [
                    TextButton(
                      onPressed: () => Navigator.pop(context, 'OK'),
                      child: const Text('OK'),
                    ),
                  ]));
    } else if (coordinatorVersion.version < clientVersion) {
      logger.w("10101 is newer than LSP: ${coordinatorVersion.version.toString()}");
    } else {
      logger.i("Client is up to date: ${clientVersion.toString()}");
    }
  } catch (e) {
    logger.e("Error getting coordinator version: ${e.toString()}");
    showDialog(
        context: shellNavigatorKey.currentContext!,
        builder: (context) => AlertDialog(
                title: const Text("Cannot reach LSP"),
                content: const Text("Please check your Internet connection.\n"
                    "Please note that without Internet access, the app "
                    "functionality is severely limited."),
                actions: [
                  TextButton(
                    onPressed: () => Navigator.pop(context, 'OK'),
                    child: const Text('OK'),
                  ),
                ]));
  }
}
