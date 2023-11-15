import 'dart:io';

import 'package:flutter/material.dart';
import 'package:get_10101/features/trade/order_change_notifier.dart';
import 'package:get_10101/features/trade/position_change_notifier.dart';
import 'package:get_10101/ffi.dart' as rust;
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/util/environment.dart';
import 'package:get_10101/util/file.dart';
import 'package:package_info_plus/package_info_plus.dart';
import 'package:path_provider/path_provider.dart';
import 'package:firebase_messaging/firebase_messaging.dart';
import 'package:get_10101/logger/logger.dart';
import 'package:provider/provider.dart';

Future<void> setConfig() async {
  bridge.Config config = Environment.parse();

  _setupRustLogging();
  _logAppSettings(config);

  // We use the app documents dir on iOS to easily access logs and DB from
  // the device. On other platforms we use the seed dir.
  String appDir = Platform.isIOS
      ? (await getApplicationDocumentsDirectory()).path
      : (await getApplicationSupportDirectory()).path;

  final seedDir = (await getApplicationSupportDirectory()).path;

  logger.i("App data will be stored in: $appDir");
  logger.i("Seed data will be stored in: $seedDir");

  final actualSeedDir = (await getActualSeedPath(config)).path;
  if (File('$actualSeedDir/db').existsSync()) {
    logger.i(
        "App has already data in the seed dir. For compatibility reasons we will not switch to the new app dir.");
    appDir = seedDir;
  }

  rust.api.setConfig(config: config, appDir: appDir, seedDir: seedDir);
}

Future<void> fullBackup() async {
  rust.api.fullBackup();
}

/// Run the backend and retry a number of times if it fails for whatever reason
Future<void> runBackend(BuildContext context) async {
  final orderChangeNotifier = context.read<OrderChangeNotifier>();
  final positionChangeNotifier = context.read<PositionChangeNotifier>();

  final seedDir = (await getApplicationSupportDirectory()).path;

  String fcmToken = await FirebaseMessaging.instance.getToken().then((value) => value ?? '');
  await _startBackend(seedDir: seedDir, fcmToken: fcmToken);

  // these notifiers depend on the backend running
  orderChangeNotifier.initialize();
  positionChangeNotifier.initialize();
}

Future<void> _startBackend({seedDir, fcmToken}) async {
  try {
    await rust.api.runInFlutter(seedDir: seedDir, fcmToken: fcmToken);
  } catch (e) {
    logger.e("Launching the app failed $e");
    await Future.delayed(const Duration(seconds: 5));
    exit(-1);
  }
}

void _setupRustLogging() {
  rust.api.initLogging().listen((event) {
    if (Platform.isAndroid || Platform.isIOS) {
      var message = event.target != ""
          ? 'r: ${event.target}: ${event.msg} ${event.data}'
          : 'r: ${event.msg} ${event.data}';
      switch (event.level) {
        case "INFO":
          logger.i(message);
        case "DEBUG":
          logger.d(message);
        case "ERROR":
          logger.e(message);
        case "WARN":
          logger.w(message);
        case "TRACE":
          logger.t(message);
        default:
          logger.d(message);
      }
    }
  });
}

Future<void> _logAppSettings(bridge.Config config) async {
  String commit = const String.fromEnvironment('COMMIT');
  if (commit.isNotEmpty) {
    logger.i("Built on commit: $commit");
  }

  String branch = const String.fromEnvironment('BRANCH');
  if (branch.isNotEmpty) {
    logger.i("Built on branch: $branch");
  }

  PackageInfo packageInfo = await PackageInfo.fromPlatform();
  logger.i("Build number: ${packageInfo.buildNumber}");
  logger.i("Build version: ${packageInfo.version}");

  logger.i("Network: ${config.network}");
  logger.i("Esplora endpoint: ${config.esploraEndpoint}");
  logger.i("Coordinator: ${config.coordinatorPubkey}@${config.host}:${config.p2PPort}");
  logger.i("Oracle endpoint: ${config.oracleEndpoint}");
  logger.i("Oracle PK: ${config.oraclePubkey}");
}
