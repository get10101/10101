import 'package:get_10101/ffi.dart' as rust;
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'dart:io';
import 'package:f_logs/f_logs.dart';
import 'package:path_provider/path_provider.dart';

class InitService {
  Future<void> setupRustLogging() async {
    rust.api.initLogging().listen((event) {
      // TODO: this should not be required if we enable mobile loggers for FLog.
      if (Platform.isAndroid || Platform.isIOS) {
        FLog.logThis(
            text: event.target != ""
                ? '${event.target}: ${event.msg} ${event.data}'
                : '${event.msg} ${event.data}',
            type: mapLogLevel(event.level));
      }
    });
  }

  LogLevel mapLogLevel(String level) {
    switch (level) {
      case "INFO":
        return LogLevel.INFO;
      case "DEBUG":
        return LogLevel.DEBUG;
      case "ERROR":
        return LogLevel.ERROR;
      case "WARN":
        return LogLevel.WARNING;
      case "TRACE":
        return LogLevel.TRACE;
      default:
        return LogLevel.DEBUG;
    }
  }

  Future<void> runBackend(bridge.Config config) async {
    final seedDir = (await getApplicationSupportDirectory()).path;

    // We use the app documents dir on iOS to easily access logs and DB from
    // the device. On other plaftorms we use the seed dir.
    String appDir = Platform.isIOS
        ? (await getApplicationDocumentsDirectory()).path
        : (await getApplicationSupportDirectory()).path;

    final network = config.network == "mainnet" ? "bitcoin" : config.network;
    if (File('$seedDir/$network/db').existsSync()) {
      FLog.info(
          text:
              "App has already data in the seed dir. For compatibility reasons we will not switch to the new app dir.");
      appDir = seedDir;
    }

    FLog.info(text: "App data will be stored in: $appDir");
    FLog.info(text: "Seed data will be stored in: $seedDir");
    await rust.api.runInFlutter(config: config, appDir: appDir, seedDir: seedDir);
  }

  Future<bridge.LastLogin> updateLastLogin() async {
    return await rust.api.updateLastLogin();
  }
}
