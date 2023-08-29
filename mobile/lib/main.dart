import 'dart:convert';
import 'dart:io';

import 'package:f_logs/f_logs.dart';
import 'package:firebase_core/firebase_core.dart';
import 'package:firebase_messaging/firebase_messaging.dart';
import 'package:flutter/material.dart';
import 'package:flutter_local_notifications/flutter_local_notifications.dart';
import 'package:flutter_native_splash/flutter_native_splash.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/common/amount_denomination_change_notifier.dart';
import 'package:get_10101/common/app_bar_wrapper.dart';
import 'package:get_10101/common/application/channel_info_service.dart';
import 'package:get_10101/common/application/event_service.dart';
import 'package:get_10101/common/channel_status_notifier.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/common/domain/service_status.dart';
import 'package:get_10101/common/service_status_notifier.dart';
import 'package:get_10101/features/stable/stable_screen.dart';
import 'package:get_10101/features/trade/application/candlestick_service.dart';
import 'package:get_10101/features/trade/application/order_service.dart';
import 'package:get_10101/features/trade/application/position_service.dart';
import 'package:get_10101/features/trade/application/trade_values_service.dart';
import 'package:get_10101/features/trade/candlestick_change_notifier.dart';
import 'package:get_10101/features/trade/domain/order.dart';
import 'package:get_10101/features/trade/domain/position.dart';
import 'package:get_10101/features/trade/domain/price.dart';
import 'package:get_10101/features/trade/order_change_notifier.dart';
import 'package:get_10101/features/trade/position_change_notifier.dart';
import 'package:get_10101/features/trade/submit_order_change_notifier.dart';
import 'package:get_10101/features/trade/trade_screen.dart';
import 'package:get_10101/features/trade/trade_theme.dart';
import 'package:get_10101/features/trade/trade_value_change_notifier.dart';
import 'package:get_10101/features/wallet/application/wallet_service.dart';
import 'package:get_10101/features/wallet/create_invoice_screen.dart';
import 'package:get_10101/features/wallet/domain/share_invoice.dart';
import 'package:get_10101/features/wallet/domain/wallet_info.dart';
import 'package:get_10101/features/wallet/scanner_screen.dart';
import 'package:get_10101/features/wallet/seed_screen.dart';
import 'package:get_10101/features/wallet/send_payment_change_notifier.dart';
import 'package:get_10101/features/wallet/send_screen.dart';
import 'package:get_10101/features/wallet/share_invoice_screen.dart';
import 'package:get_10101/features/wallet/wallet_change_notifier.dart';
import 'package:get_10101/features/wallet/wallet_screen.dart';
import 'package:get_10101/features/wallet/wallet_theme.dart';
import 'package:get_10101/features/welcome/welcome_screen.dart';
import 'package:get_10101/ffi.dart' as rust;
import 'package:get_10101/firebase_options.dart';
import 'package:get_10101/util/constants.dart';
import 'package:get_10101/util/environment.dart';
import 'package:get_10101/util/preferences.dart';
import 'package:go_router/go_router.dart';
import 'package:http/http.dart' as http;
import 'package:package_info_plus/package_info_plus.dart';
import 'package:path_provider/path_provider.dart';
import 'package:provider/provider.dart';
import 'package:version/version.dart';

import 'features/stable/stable_value_change_notifier.dart';

final GlobalKey<NavigatorState> _rootNavigatorKey = GlobalKey<NavigatorState>(debugLabel: 'root');
final GlobalKey<NavigatorState> _shellNavigatorKey = GlobalKey<NavigatorState>(debugLabel: 'shell');

void main() {
  WidgetsBinding widgetsBinding = WidgetsFlutterBinding.ensureInitialized();
  FlutterNativeSplash.preserve(widgetsBinding: widgetsBinding);

  setupFlutterLogs();

  const ChannelInfoService channelInfoService = ChannelInfoService();
  var tradeValuesService = TradeValuesService();

  runApp(MultiProvider(providers: [
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
    Provider(create: (context) => Environment.parse()),
    Provider(create: (context) => channelInfoService)
  ], child: const TenTenOneApp()));
}

void setupFlutterLogs() {
  final config = FLog.getDefaultConfigurations();
  config.activeLogLevel = LogLevel.TRACE;
  config.formatType = FormatType.FORMAT_CUSTOM;
  config.timestampFormat = 'yyyy-MM-dd HH:mm:ss.SSS';
  config.fieldOrderFormatCustom = [
    FieldName.TIMESTAMP,
    FieldName.LOG_LEVEL,
    FieldName.TEXT,
    FieldName.STACKTRACE
  ];
  config.customClosingDivider = " ";
  config.customOpeningDivider = "";

  FLog.applyConfigurations(config);
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
      navigatorKey: _rootNavigatorKey,
      initialLocation: WalletScreen.route,
      routes: <RouteBase>[
        ShellRoute(
          navigatorKey: _shellNavigatorKey,
          builder: (BuildContext context, GoRouterState state, Widget child) {
            return ScaffoldWithNavBar(
              child: child,
            );
          },
          routes: <RouteBase>[
            GoRoute(
              path: WalletScreen.route,
              builder: (BuildContext context, GoRouterState state) {
                return const WalletScreen();
              },
              routes: <RouteBase>[
                GoRoute(
                  path: SendScreen.subRouteName,
                  // Use root navigator so the screen overlays the application shell
                  parentNavigatorKey: _rootNavigatorKey,
                  builder: (BuildContext context, GoRouterState state) {
                    return const SendScreen();
                  },
                ),
                GoRoute(
                  path: SeedScreen.subRouteName,
                  // Use root navigator so the screen overlays the application shell
                  parentNavigatorKey: _rootNavigatorKey,
                  builder: (BuildContext context, GoRouterState state) {
                    return const SeedScreen();
                  },
                ),
                GoRoute(
                    path: CreateInvoiceScreen.subRouteName,
                    // Use root navigator so the screen overlays the application shell
                    parentNavigatorKey: _rootNavigatorKey,
                    builder: (BuildContext context, GoRouterState state) {
                      return const CreateInvoiceScreen();
                    },
                    routes: [
                      GoRoute(
                        path: ShareInvoiceScreen.subRouteName,
                        // Use root navigator so the screen overlays the application shell
                        parentNavigatorKey: _rootNavigatorKey,
                        builder: (BuildContext context, GoRouterState state) {
                          return ShareInvoiceScreen(invoice: state.extra as ShareInvoice);
                        },
                      ),
                    ]),
                GoRoute(
                  path: ScannerScreen.subRouteName,
                  parentNavigatorKey: _rootNavigatorKey,
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
            parentNavigatorKey: _rootNavigatorKey,
            builder: (BuildContext context, GoRouterState state) {
              return const WelcomeScreen();
            },
            routes: const []),
      ],
      redirect: (BuildContext context, GoRouterState state) async {
        // TODO: It's not optimal that we read this from shared prefs every time, should probably be set through a provider
        final hasEmailAddress = await Preferences.instance.hasEmailAddress();
        if (!hasEmailAddress) {
          FLog.info(text: "adding the email...");
          return WelcomeScreen.route;
        }

        return null;
      });

  @override
  void initState() {
    super.initState();

    final config = context.read<bridge.Config>();

    init(config);

    WidgetsBinding.instance.addPostFrameCallback((_) async {
      await initFirebase();
      await requestNotificationPermission();
      final flutterLocalNotificationsPlugin = initLocalNotifications();
      await configureFirebase(flutterLocalNotificationsPlugin);

      PackageInfo packageInfo = await PackageInfo.fromPlatform();

      final messenger = scaffoldMessengerKey.currentState!;

      try {
        final response = await http.get(
          Uri.parse('http://${config.host}:${config.httpPort}/api/version'),
        );

        final clientVersion = Version.parse(packageInfo.version);
        final coordinatorVersion = Version.parse(jsonDecode(response.body));
        FLog.info(text: "Coordinator version: ${coordinatorVersion.toString()}");

        if (coordinatorVersion > clientVersion) {
          FLog.warning(text: "Client out of date. Current version: ${clientVersion.toString()}");
          showDialog(
              context: _shellNavigatorKey.currentContext!,
              builder: (context) => AlertDialog(
                      title: const Text("Update available"),
                      content: Text("A new version of 10101 is available: "
                          "${coordinatorVersion.toString()}.\n\n"
                          "Please note that if you do not update 10101, the app"
                          " may not function properly."),
                      actions: [
                        TextButton(
                          onPressed: () => Navigator.pop(context, 'OK'),
                          child: const Text('OK'),
                        ),
                      ]));
        } else {
          FLog.info(text: "Client up to date");
        }
      } catch (e) {
        FLog.error(text: "Error getting coordinator version: ${e.toString()}");
        messenger.showSnackBar(const SnackBar(content: Text("Coordinator offline")));
      }
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
    final walletChangeNotifier = context.read<WalletChangeNotifier>();

    try {
      setupRustLogging();

      subscribeToNotifiers(context);

      await runBackend(config);
      FLog.info(text: "Backend started");

      await orderChangeNotifier.initialize();
      await positionChangeNotifier.initialize();
      await candlestickChangeNotifier.initialize(config.network);

      await logAppSettings(config);

      final lastLogin = await rust.api.updateLastLogin();
      FLog.debug(text: "Last login was at ${lastLogin.date}");
    } on FfiException catch (error) {
      FLog.error(text: "Failed to initialise: Error: ${error.message}", exception: error);
    } catch (error) {
      FLog.error(text: "Failed to initialise: $error", exception: error);
    } finally {
      FlutterNativeSplash.remove();
    }
    await walletChangeNotifier.refreshWalletInfo();
  }

  setupRustLogging() {
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
    FLog.info(text: "Built on commit: $commit");
  }

  String branch = const String.fromEnvironment('BRANCH');
  if (branch.isNotEmpty) {
    FLog.info(text: "Built on branch: $branch");
  }

  PackageInfo packageInfo = await PackageInfo.fromPlatform();
  FLog.info(text: "Build number: ${packageInfo.buildNumber}");
  FLog.info(text: "Build version: ${packageInfo.version}");

  FLog.info(text: "Network: ${config.network}");
  FLog.info(text: "Esplora endpoint: ${config.esploraEndpoint}");
  FLog.info(text: "Coordinator: ${config.coordinatorPubkey}@${config.host}:${config.p2PPort}");
  FLog.info(text: "Oracle endpoint: ${config.oracleEndpoint}");
  FLog.info(text: "Oracle PK: ${config.oraclePubkey}");

  String nodeId = rust.api.getNodeId();
  FLog.info(text: "Node ID: $nodeId");
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

  channelStatusNotifier.subscribe(eventService);

  eventService.subscribe(
      AnonSubscriber((event) => FLog.info(text: event.field0)), const bridge.Event.log(""));
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
  await startBackend(config: config, appDir: appDir, seedDir: seedDir);
}

FlutterLocalNotificationsPlugin initLocalNotifications() {
  final flutterLocalNotificationsPlugin = FlutterLocalNotificationsPlugin();
  const androidSettings = AndroidInitializationSettings('@mipmap/ic_launcher');
  const darwinSettings = DarwinInitializationSettings();
  const initializationSettings =
      InitializationSettings(android: androidSettings, macOS: darwinSettings, iOS: darwinSettings);
  flutterLocalNotificationsPlugin.initialize(initializationSettings);
  return flutterLocalNotificationsPlugin;
}

Future<void> initFirebase() async {
  await Firebase.initializeApp(
    options: DefaultFirebaseOptions.currentPlatform,
  );
}

Future<void> configureFirebase(FlutterLocalNotificationsPlugin localNotifications) async {
  // Configure message handler
  FirebaseMessaging.onMessage.listen((RemoteMessage message) {
    // TODO: Handle messages from Firebase
    FLog.debug(text: "Firebase message received: ${message.data}");

    if (message.notification != null) {
      FLog.debug(text: "Message also contained a notification: ${message.notification}");
      showNotification(message.notification!.toMap(), localNotifications);
    }
  });

  // Setup the message handler when the app is not running
  FirebaseMessaging.onBackgroundMessage(_firebaseMessagingBackgroundHandler);

  FirebaseMessaging messaging = FirebaseMessaging.instance;
  // Subscribe to topic "all" to receive all messages
  messaging.subscribeToTopic('all');
}

/// Ask the user for permission to send notifications via Firebase
Future<void> requestNotificationPermission() async {
  FirebaseMessaging messaging = FirebaseMessaging.instance;
  final token = await messaging.getToken();
  FLog.debug(text: "Firebase token: $token");

  NotificationSettings settings = await messaging.requestPermission(
    alert: true,
    announcement: false,
    badge: true,
    carPlay: false,
    criticalAlert: false,
    provisional: false,
    sound: true,
  );

  FLog.info(text: "User granted permission: ${settings.authorizationStatus}");
}

/// Start the backend and retry a number of times if it fails for whatever reason
Future<void> startBackend({config, appDir, seedDir}) async {
  int retries = 3;

  for (int i = 0; i < retries; i++) {
    try {
      await rust.api.runInFlutter(config: config, appDir: appDir, seedDir: seedDir);
      break; // If successful, exit loop
    } catch (e) {
      FLog.info(text: "Attempt ${i + 1} failed: $e");
      if (i < retries - 1) {
        await Future.delayed(const Duration(seconds: 5));
      } else {
        FLog.error(text: "Max retries reached, backend could not start.");
        exit(-1);
      }
    }
  }
}

/// Display notification inside the `message` using the local notification plugin
void showNotification(
    Map<String, dynamic> message, FlutterLocalNotificationsPlugin localNotifications) async {
  const androidPlatformChannelSpecifics = AndroidNotificationDetails(
    'channel_id',
    'channel_name',
    channelDescription: 'channel_description',
    importance: Importance.max,
    priority: Priority.high,
  );

  const darwinPlatformChannelSpecifics = DarwinNotificationDetails(
    presentAlert: true,
    presentBadge: true,
    presentSound: true,
  );

  const platformChannelSpecifics = NotificationDetails(
    android: androidPlatformChannelSpecifics,
    iOS: darwinPlatformChannelSpecifics,
    macOS: darwinPlatformChannelSpecifics,
  );

  FLog.debug(text: "Showing notification: ${message['title']} with body ${message['body']}");

  await localNotifications.show(
    0,
    message['title'],
    message['body'],
    platformChannelSpecifics,
    payload: 'item x',
  );
}

/// Handle background messages (when the app is not running)
Future<void> _firebaseMessagingBackgroundHandler(RemoteMessage message) async {
  FLog.debug(text: "Handling a background message: ${message.messageId}");

  await Firebase.initializeApp();
  final localNotifications = initLocalNotifications();

  if (message.notification != null) {
    FLog.debug(text: "Message also contained a notification: ${message.notification}");
    showNotification(message.notification!.toMap(), localNotifications);
  }
}
