import 'dart:io';

import 'package:flutter/material.dart';
import 'package:get_10101/features/trade/candlestick_change_notifier.dart';
import 'package:get_10101/features/trade/order_change_notifier.dart';
import 'package:get_10101/features/trade/position_change_notifier.dart';
import 'package:get_10101/ffi.dart' as rust;
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/logger/logger.dart';
import 'package:get_10101/util/environment.dart';
import 'package:get_10101/util/file.dart';
import 'package:path_provider/path_provider.dart';
import 'package:firebase_messaging/firebase_messaging.dart';
import 'package:provider/provider.dart';

Future<void> setConfig() async {
  bridge.Config config = Environment.parse();

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
  context.read<CandlestickChangeNotifier>().initialize();
  final orderChangeNotifier = context.read<OrderChangeNotifier>();
  final positionChangeNotifier = context.read<PositionChangeNotifier>();

  final seedDir = (await getApplicationSupportDirectory()).path;

  String fcmToken;
  try {
    fcmToken = await FirebaseMessaging.instance.getToken().then((value) => value ?? '');
  } catch (e) {
    logger.e("Error fetching FCM token: $e");
    fcmToken = '';
  }

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
