import 'dart:convert';
import 'dart:io';

import 'package:firebase_messaging/firebase_messaging.dart';
import 'package:flutter/material.dart';
import 'package:flutter_native_splash/flutter_native_splash.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/common/amount_denomination_change_notifier.dart';
import 'package:get_10101/common/app_bar_wrapper.dart';
import 'package:get_10101/common/application/channel_info_service.dart';
import 'package:get_10101/common/application/event_service.dart';
import 'package:get_10101/common/channel_status_notifier.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/common/domain/background_task.dart';
import 'package:get_10101/common/domain/service_status.dart';
import 'package:get_10101/common/global_keys.dart';
import 'package:get_10101/common/loading_screen.dart';
import 'package:get_10101/common/recover_dlc_change_notifier.dart';
import 'package:get_10101/common/service_status_notifier.dart';
import 'package:get_10101/features/stable/stable_screen.dart';
import 'package:get_10101/features/trade/application/candlestick_service.dart';
import 'package:get_10101/features/trade/application/order_service.dart';
import 'package:get_10101/features/trade/application/position_service.dart';
import 'package:get_10101/features/trade/application/trade_values_service.dart';
import 'package:get_10101/features/trade/async_order_change_notifier.dart';
import 'package:get_10101/features/trade/candlestick_change_notifier.dart';
import 'package:get_10101/features/trade/domain/order.dart';
import 'package:get_10101/features/trade/domain/position.dart';
import 'package:get_10101/features/trade/domain/price.dart';
import 'package:get_10101/features/trade/order_change_notifier.dart';
import 'package:get_10101/features/trade/position_change_notifier.dart';
import 'package:get_10101/features/trade/rollover_change_notifier.dart';
import 'package:get_10101/features/trade/submit_order_change_notifier.dart';
import 'package:get_10101/features/trade/trade_screen.dart';
import 'package:get_10101/features/trade/trade_theme.dart';
import 'package:get_10101/features/trade/trade_value_change_notifier.dart';
import 'package:get_10101/features/wallet/application/faucet_service.dart';
import 'package:get_10101/features/wallet/application/wallet_service.dart';
import 'package:get_10101/features/wallet/create_invoice_screen.dart';
import 'package:get_10101/features/wallet/domain/share_payment_request.dart';
import 'package:get_10101/features/wallet/domain/wallet_info.dart';
import 'package:get_10101/features/wallet/onboarding/onboarding_screen.dart';
import 'package:get_10101/features/wallet/payment_claimed_change_notifier.dart';
import 'package:get_10101/features/wallet/scanner_screen.dart';
import 'package:get_10101/features/wallet/seed_screen.dart';
import 'package:get_10101/features/wallet/send_payment_change_notifier.dart';
import 'package:get_10101/features/wallet/send_payment_screen.dart';
import 'package:get_10101/features/wallet/share_payment_request_screen.dart';
import 'package:get_10101/features/wallet/wallet_change_notifier.dart';
import 'package:get_10101/features/wallet/wallet_screen.dart';
import 'package:get_10101/features/wallet/wallet_theme.dart';
import 'package:get_10101/features/welcome/welcome_screen.dart';
import 'package:get_10101/ffi.dart' as rust;
import 'package:get_10101/util/constants.dart';
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

import 'features/stable/stable_value_change_notifier.dart';

void main() async {
  WidgetsBinding widgetsBinding = WidgetsFlutterBinding.ensureInitialized();
  FlutterNativeSplash.preserve(widgetsBinding: widgetsBinding);

  await initFirebase();

  const ChannelInfoService channelInfoService = ChannelInfoService();
  var tradeValuesService = TradeValuesService();
  var config = Environment.parse();

  var providers = [
    ChangeNotifierProvider(create: (context) {
      return TradeValuesChangeNotifier(tradeValuesService, channelInfoService);
    }),
    ChangeNotifierProvider(create: (context) {
      return StableValuesChangeNotifier(tradeValuesService, channelInfoService);
    }),
    ChangeNotifierProvider(create: (context) => AmountDenominationChangeNotifier()),
    ChangeNotifierProvider(create: (context) => SubmitOrderChangeNotifier(OrderService())),
    ChangeNotifierProvider(create: (context) => OrderChangeNotifier(OrderService())),
    ChangeNotifierProvider(create: (context) => PositionChangeNotifier(PositionService())),
    ChangeNotifierProvider(create: (context) => WalletChangeNotifier(const WalletService())),
    ChangeNotifierProvider(create: (context) => SendPaymentChangeNotifier(const WalletService())),
    ChangeNotifierProvider(
        create: (context) => CandlestickChangeNotifier(const CandlestickService())),
    ChangeNotifierProvider(create: (context) => ServiceStatusNotifier()),
    ChangeNotifierProvider(create: (context) => ChannelStatusNotifier()),
    ChangeNotifierProvider(create: (context) => AsyncOrderChangeNotifier(OrderService())),
    ChangeNotifierProvider(create: (context) => RolloverChangeNotifier()),
    ChangeNotifierProvider(create: (context) => RecoverDlcChangeNotifier()),
    ChangeNotifierProvider(create: (context) => PaymentClaimedChangeNotifier()),
    Provider(create: (context) => config),
    Provider(create: (context) => channelInfoService)
  ];

  if (config.network == "regtest") {
    providers.add(Provider(create: (context) => FaucetService()));
  }

  runApp(MultiProvider(providers: providers, child: const TenTenOneApp()));
}

class TenTenOneApp extends StatefulWidget {
  const TenTenOneApp({Key? key}) : super(key: key);

  @override
  State<TenTenOneApp> createState() => _TenTenOneAppState();
}

class _TenTenOneAppState extends State<TenTenOneApp> {
  final GlobalKey<ScaffoldMessengerState> scaffoldMessengerKey =
      GlobalKey<ScaffoldMessengerState>();

  final GoRouter _router = GoRouter(
      navigatorKey: rootNavigatorKey,
      initialLocation: LoadingScreen.route,
      routes: <RouteBase>[
        ShellRoute(
          navigatorKey: shellNavigatorKey,
          builder: (BuildContext context, GoRouterState state, Widget child) {
            return ScaffoldWithNavBar(
              child: child,
            );
          },
          routes: <RouteBase>[
            GoRoute(
              path: LoadingScreen.route,
              builder: (BuildContext context, GoRouterState state) {
                return const LoadingScreen();
              },
            ),
            GoRoute(
              path: WalletScreen.route,
              builder: (BuildContext context, GoRouterState state) {
                return const WalletScreen();
              },
              routes: <RouteBase>[
                GoRoute(
                  path: SendPaymentScreen.subRouteName,
                  // Use root navigator so the screen overlays the application shell
                  parentNavigatorKey: rootNavigatorKey,
                  builder: (BuildContext context, GoRouterState state) {
                    return const SendPaymentScreen();
                  },
                ),
                GoRoute(
                  path: SeedScreen.subRouteName,
                  // Use root navigator so the screen overlays the application shell
                  parentNavigatorKey: rootNavigatorKey,
                  builder: (BuildContext context, GoRouterState state) {
                    return const SeedScreen();
                  },
                ),
                GoRoute(
                    path: OnboardingScreen.subRouteName,
                    parentNavigatorKey: rootNavigatorKey,
                    builder: (BuildContext context, GoRouterState state) {
                      return const OnboardingScreen();
                    }),
                GoRoute(
                  path: CreateInvoiceScreen.subRouteName,
                  // Use root navigator so the screen overlays the application shell
                  parentNavigatorKey: rootNavigatorKey,
                  builder: (BuildContext context, GoRouterState state) {
                    return const CreateInvoiceScreen();
                  },
                ),
                GoRoute(
                  path: SharePaymentRequestScreen.subRouteName,
                  // Use root navigator so the screen overlays the application shell
                  parentNavigatorKey: rootNavigatorKey,
                  builder: (BuildContext context, GoRouterState state) {
                    return SharePaymentRequestScreen(request: state.extra as SharePaymentRequest);
                  },
                ),
                GoRoute(
                  path: ScannerScreen.subRouteName,
                  parentNavigatorKey: rootNavigatorKey,
                  builder: (BuildContext context, GoRouterState state) {
                    return const ScannerScreen();
                  },
                ),
              ],
            ),
            GoRoute(
                path: StableScreen.route,
                builder: (BuildContext context, GoRouterState state) {
                  return const StableScreen();
                }),
            GoRoute(
              path: TradeScreen.route,
              builder: (BuildContext context, GoRouterState state) {
                return const TradeScreen();
              },
              routes: const [],
            ),
          ],
        ),
        GoRoute(
            path: WelcomeScreen.route,
            parentNavigatorKey: rootNavigatorKey,
            builder: (BuildContext context, GoRouterState state) {
              return const WelcomeScreen();
            },
            routes: const []),
      ]);

  @override
  void initState() {
    super.initState();

    final config = context.read<bridge.Config>();

    init(config);

    WidgetsBinding.instance.addPostFrameCallback((_) async {
      await compareCoordinatorVersion(config);
    });
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

// Wrapper for the main application screens
class ScaffoldWithNavBar extends StatelessWidget {
  const ScaffoldWithNavBar({
    required this.child,
    Key? key,
  }) : super(key: key);

  final Widget child;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: child,
      appBar: const PreferredSize(
          preferredSize: Size.fromHeight(40), child: SafeArea(child: AppBarWrapper())),
      bottomNavigationBar: BottomNavigationBar(
        items: <BottomNavigationBarItem>[
          BottomNavigationBarItem(
            icon: Container(key: tabStable, child: const Icon(Icons.currency_exchange)),
            label: StableScreen.label,
          ),
          BottomNavigationBarItem(
            icon: Container(key: tabWallet, child: const Icon(Icons.wallet)),
            label: WalletScreen.label,
          ),
          BottomNavigationBarItem(
            icon: Container(key: tabTrade, child: const Icon(Icons.bar_chart)),
            label: TradeScreen.label,
          ),
        ],
        currentIndex: _calculateSelectedIndex(context),
        onTap: (int idx) => _onItemTapped(idx, context),
      ),
    );
  }

  static int _calculateSelectedIndex(BuildContext context) {
    final String location = GoRouterState.of(context).location;
    if (location.startsWith(StableScreen.route)) {
      return 0;
    }
    if (location.startsWith(WalletScreen.route)) {
      return 1;
    }
    if (location.startsWith(TradeScreen.route)) {
      return 2;
    }
    return 1;
  }

  void _onItemTapped(int index, BuildContext context) {
    switch (index) {
      case 0:
        GoRouter.of(context).go(StableScreen.route);
        break;
      case 1:
        GoRouter.of(context).go(WalletScreen.route);
        break;
      case 2:
        GoRouter.of(context).go(TradeScreen.route);
        break;
    }
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

/// Forward the events from change notifiers to the Event service
void subscribeToNotifiers(BuildContext context) {
  // TODO: Move this code into an "InitService" or similar; we should not have bridge code in the widget

  final EventService eventService = EventService.create();

  final orderChangeNotifier = context.read<OrderChangeNotifier>();
  final positionChangeNotifier = context.read<PositionChangeNotifier>();
  final walletChangeNotifier = context.read<WalletChangeNotifier>();
  final tradeValuesChangeNotifier = context.read<TradeValuesChangeNotifier>();
  final submitOrderChangeNotifier = context.read<SubmitOrderChangeNotifier>();
  final serviceStatusNotifier = context.read<ServiceStatusNotifier>();
  final channelStatusNotifier = context.read<ChannelStatusNotifier>();
  final stableValuesChangeNotifier = context.read<StableValuesChangeNotifier>();
  final asyncOrderChangeNotifier = context.read<AsyncOrderChangeNotifier>();
  final rolloverChangeNotifier = context.read<RolloverChangeNotifier>();
  final recoverDlcChangeNotifier = context.read<RecoverDlcChangeNotifier>();
  final paymentClaimedChangeNotifier = context.read<PaymentClaimedChangeNotifier>();

  eventService.subscribe(
      orderChangeNotifier, bridge.Event.orderUpdateNotification(Order.apiDummy()));

  eventService.subscribe(
      submitOrderChangeNotifier, bridge.Event.orderUpdateNotification(Order.apiDummy()));

  eventService.subscribe(
      positionChangeNotifier, bridge.Event.positionUpdateNotification(Position.apiDummy()));

  eventService.subscribe(
      positionChangeNotifier,
      const bridge.Event.positionClosedNotification(
          bridge.PositionClosed(contractSymbol: bridge.ContractSymbol.BtcUsd)));

  eventService.subscribe(
      walletChangeNotifier, bridge.Event.walletInfoUpdateNotification(WalletInfo.apiDummy()));

  eventService.subscribe(
      tradeValuesChangeNotifier, bridge.Event.priceUpdateNotification(Price.apiDummy()));

  eventService.subscribe(
      stableValuesChangeNotifier, bridge.Event.priceUpdateNotification(Price.apiDummy()));

  eventService.subscribe(
      positionChangeNotifier, bridge.Event.priceUpdateNotification(Price.apiDummy()));

  eventService.subscribe(
      serviceStatusNotifier, bridge.Event.serviceHealthUpdate(serviceUpdateApiDummy()));

  eventService.subscribe(
      asyncOrderChangeNotifier, bridge.Event.orderUpdateNotification(Order.apiDummy()));
  eventService.subscribe(
      asyncOrderChangeNotifier, bridge.Event.backgroundNotification(AsyncTrade.apiDummy()));

  eventService.subscribe(
      rolloverChangeNotifier, bridge.Event.backgroundNotification(Rollover.apiDummy()));

  eventService.subscribe(
      recoverDlcChangeNotifier, bridge.Event.backgroundNotification(RecoverDlc.apiDummy()));

  eventService.subscribe(paymentClaimedChangeNotifier, const bridge.Event.paymentClaimed());

  channelStatusNotifier.subscribe(eventService);

  eventService.subscribe(
      AnonSubscriber((event) => logger.i(event.field0)), const bridge.Event.log(""));
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
