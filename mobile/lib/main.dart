import 'dart:convert';
import 'dart:io';

import 'package:firebase_messaging/firebase_messaging.dart';
import 'package:flutter/material.dart';
import 'package:flutter_native_splash/flutter_native_splash.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/common/color.dart';
import 'package:get_10101/common/global_keys.dart';
import 'package:get_10101/common/init_service.dart';
import 'package:get_10101/common/routes.dart';
import 'package:get_10101/features/trade/candlestick_change_notifier.dart';
import 'package:get_10101/features/trade/order_change_notifier.dart';
import 'package:get_10101/features/trade/position_change_notifier.dart';
import 'package:get_10101/features/trade/trade_theme.dart';
import 'package:get_10101/features/wallet/wallet_theme.dart';
import 'package:get_10101/ffi.dart' as rust;
import 'package:get_10101/util/coordinator_version.dart';
import 'package:get_10101/util/environment.dart';
import 'package:get_10101/util/notifications.dart';
import 'package:go_router/go_router.dart';
import 'package:http/http.dart' as http;
import 'package:package_info_plus/package_info_plus.dart';
import 'package:path_provider/path_provider.dart';
import 'package:provider/provider.dart';
import 'package:version/version.dart';
import 'package:get_10101/logger/logger.dart';

void main() async {
  WidgetsBinding widgetsBinding = WidgetsFlutterBinding.ensureInitialized();
  FlutterNativeSplash.preserve(widgetsBinding: widgetsBinding);

  await initFirebase();

  var config = Environment.parse();

  var providers = createProviders(config);
  runApp(MultiProvider(providers: providers, child: const TenTenOneApp()));
}

class TenTenOneApp extends StatefulWidget {
  const TenTenOneApp({Key? key}) : super(key: key);

  @override
  State<TenTenOneApp> createState() => _TenTenOneAppState();
}

class _TenTenOneAppState extends State<TenTenOneApp> with WidgetsBindingObserver {
  final GlobalKey<ScaffoldMessengerState> scaffoldMessengerKey =
      GlobalKey<ScaffoldMessengerState>();

  final GoRouter _router = createRouter();

  @override
  void initState() {
    super.initState();

    WidgetsBinding.instance.addObserver(this);

    final config = context.read<bridge.Config>();

    init(config);

    WidgetsBinding.instance.addPostFrameCallback((_) async {
      await compareCoordinatorVersion(config);
    });
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    logger.d("AppLifecycleState changed to: $state");
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    MaterialColor swatch = tenTenOnePurple;

    return MaterialApp.router(
      title: "10101",
      scaffoldMessengerKey: scaffoldMessengerKey,
      theme: ThemeData(
        primarySwatch: swatch,
        iconTheme: IconThemeData(
          color: tenTenOnePurple.shade800,
          size: 32,
        ),
        extensions: <ThemeExtension<dynamic>>[
          const TradeTheme(),
          WalletTheme(colors: ColorScheme.fromSwatch(primarySwatch: swatch)),
        ],
      ),
      routerConfig: _router,
      debugShowCheckedModeBanner: false,
    );
  }

  Future<void> init(bridge.Config config) async {
    final orderChangeNotifier = context.read<OrderChangeNotifier>();
    final positionChangeNotifier = context.read<PositionChangeNotifier>();
    final candlestickChangeNotifier = context.read<CandlestickChangeNotifier>();

    try {
      setupRustLogging();

      subscribeToNotifiers(context);

      await runBackend(config);
      logger.i("Backend started");

      orderChangeNotifier.initialize();
      positionChangeNotifier.initialize();
      candlestickChangeNotifier.initialize();

      logAppSettings(config);

      rust.api
          .updateLastLogin()
          .then((lastLogin) => logger.d("Last login was at ${lastLogin.date}"));
    } on FfiException catch (error) {
      logger.e("Failed to initialise: Error: ${error.message}", error: error);
    } catch (error) {
      logger.e("Failed to initialise: $error", error: error);
    } finally {
      FlutterNativeSplash.remove();
    }
  }

  void setupRustLogging() {
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
}

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

Future<void> logAppSettings(bridge.Config config) async {
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

  try {
    String nodeId = rust.api.getNodeId();
    logger.i("Node ID: $nodeId");
  } catch (e) {
    logger.e("Failed to get node ID: $e");
  }
}

Future<void> runBackend(bridge.Config config) async {
  final seedDir = (await getApplicationSupportDirectory()).path;

  // We use the app documents dir on iOS to easily access logs and DB from
  // the device. On other platforms we use the seed dir.
  String appDir = Platform.isIOS
      ? (await getApplicationDocumentsDirectory()).path
      : (await getApplicationSupportDirectory()).path;

  final network = config.network == "mainnet" ? "bitcoin" : config.network;
  if (File('$seedDir/$network/db').existsSync()) {
    logger.i(
        "App has already data in the seed dir. For compatibility reasons we will not switch to the new app dir.");
    appDir = seedDir;
  }

  String fcmToken;
  try {
    fcmToken = await FirebaseMessaging.instance.getToken().then((value) => value ?? '');
  } catch (e) {
    logger.e("Error fetching FCM token: $e");
    fcmToken = '';
  }

  logger.i("App data will be stored in: $appDir");
  logger.i("Seed data will be stored in: $seedDir");
  await startBackend(config: config, appDir: appDir, seedDir: seedDir, fcmToken: fcmToken);
}

/// Start the backend and retry a number of times if it fails for whatever reason
Future<void> startBackend({config, appDir, seedDir, fcmToken}) async {
  int retries = 3;

  for (int i = 0; i < retries; i++) {
    try {
      await rust.api
          .runInFlutter(config: config, appDir: appDir, seedDir: seedDir, fcmToken: fcmToken);
      break; // If successful, exit loop
    } catch (e) {
      logger.i("Attempt ${i + 1} failed: $e");
      if (i < retries - 1) {
        await Future.delayed(const Duration(seconds: 5));
      } else {
        logger.e("Max retries reached, backend could not start.");
        exit(-1);
      }
    }
  }
}
