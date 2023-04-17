import 'package:f_logs/f_logs.dart';
import 'package:flutter/material.dart';
import 'dart:io';
import 'package:flutter_native_splash/flutter_native_splash.dart';
import 'package:flutter_rust_bridge/flutter_rust_bridge.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/common/application/event_service.dart';
import 'package:get_10101/features/trade/application/candlestick_service.dart';
import 'package:get_10101/features/trade/application/order_service.dart';
import 'package:get_10101/features/trade/application/position_service.dart';
import 'package:get_10101/features/trade/application/trade_values_service.dart';
import 'package:get_10101/features/trade/candlestick_change_notifier.dart';
import 'package:get_10101/features/trade/domain/position.dart';
import 'package:get_10101/features/trade/order_change_notifier.dart';
import 'package:get_10101/features/trade/position_change_notifier.dart';
import 'package:get_10101/features/trade/submit_order_change_notifier.dart';
import 'package:get_10101/features/trade/trade_value_change_notifier.dart';
import 'package:get_10101/features/trade/settings_screen.dart';
import 'package:get_10101/features/trade/trade_theme.dart';
import 'package:get_10101/features/wallet/application/wallet_service.dart';
import 'package:get_10101/features/wallet/create_invoice_screen.dart';
import 'package:get_10101/features/wallet/seed_screen.dart';
import 'package:get_10101/features/wallet/share_invoice_screen.dart';
import 'package:get_10101/features/wallet/scanner_screen.dart';
import 'package:get_10101/features/wallet/send_screen.dart';
import 'package:get_10101/features/wallet/settings_screen.dart';
import 'package:get_10101/features/trade/trade_screen.dart';
import 'package:get_10101/features/wallet/wallet_change_notifier.dart';
import 'package:get_10101/features/wallet/wallet_screen.dart';
import 'package:get_10101/common/app_bar_wrapper.dart';
import 'package:get_10101/features/wallet/wallet_theme.dart';
import 'package:get_10101/util/constants.dart';
import 'package:get_10101/util/environment.dart';
import 'package:go_router/go_router.dart';
import 'package:path_provider/path_provider.dart';
import 'package:provider/provider.dart';
import 'common/amount_denomination_change_notifier.dart';
import 'features/trade/domain/order.dart';
import 'features/trade/domain/price.dart';
import 'features/wallet/domain/wallet_info.dart';
import 'ffi.dart' as rust;

final GlobalKey<NavigatorState> _rootNavigatorKey = GlobalKey<NavigatorState>(debugLabel: 'root');
final GlobalKey<NavigatorState> _shellNavigatorKey = GlobalKey<NavigatorState>(debugLabel: 'shell');

void main() {
  WidgetsBinding widgetsBinding = WidgetsFlutterBinding.ensureInitialized();
  FlutterNativeSplash.preserve(widgetsBinding: widgetsBinding);

  final config = FLog.getDefaultConfigurations();
  config.activeLogLevel = LogLevel.DEBUG;
  config.formatType = FormatType.FORMAT_CUSTOM;
  config.timestampFormat = 'yyyy-MM-dd HH:mm:ss.SSS';
  config.fieldOrderFormatCustom = [
    FieldName.TIMESTAMP,
    FieldName.LOG_LEVEL,
    FieldName.TEXT,
    FieldName.STACKTRACE
  ];
  config.customClosingDivider = "";
  config.customOpeningDivider = "| ";

  FLog.applyConfigurations(config);
  runApp(MultiProvider(providers: [
    ChangeNotifierProvider(create: (context) => TradeValuesChangeNotifier(TradeValuesService())),
    ChangeNotifierProvider(create: (context) => AmountDenominationChangeNotifier()),
    ChangeNotifierProvider(create: (context) => SubmitOrderChangeNotifier(OrderService())),
    ChangeNotifierProvider(create: (context) => OrderChangeNotifier(OrderService())),
    ChangeNotifierProvider(create: (context) => PositionChangeNotifier(PositionService())),
    ChangeNotifierProvider(create: (context) => WalletChangeNotifier(const WalletService())),
    ChangeNotifierProvider(
        create: (context) => CandlestickChangeNotifier(const CandlestickService())),
    Provider(create: (context) => Environment.parse()),
  ], child: const TenTenOneApp()));
}

class TenTenOneApp extends StatefulWidget {
  const TenTenOneApp({Key? key}) : super(key: key);

  @override
  State<TenTenOneApp> createState() => _TenTenOneAppState();
}

class _TenTenOneAppState extends State<TenTenOneApp> {
  final GoRouter _router = GoRouter(
    navigatorKey: _rootNavigatorKey,
    initialLocation: WalletScreen.route,
    routes: <RouteBase>[
      ShellRoute(
        navigatorKey: _shellNavigatorKey,
        builder: (BuildContext context, GoRouterState state, Widget child) {
          return ScaffoldWithNavBar(child: child);
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
                        return ShareInvoiceScreen(invoice: state.extra as String);
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
              GoRoute(
                  path: WalletSettingsScreen.subRouteName,
                  parentNavigatorKey: _rootNavigatorKey,
                  builder: (BuildContext context, GoRouterState state) {
                    return const WalletSettingsScreen();
                  })
            ],
          ),
          GoRoute(
            path: TradeScreen.route,
            builder: (BuildContext context, GoRouterState state) {
              return const TradeScreen();
            },
            routes: <RouteBase>[
              GoRoute(
                  path: TradeSettingsScreen.subRouteName,
                  parentNavigatorKey: _rootNavigatorKey,
                  builder: (BuildContext context, GoRouterState state) {
                    return const TradeSettingsScreen();
                  })
            ],
          ),
        ],
      ),
    ],
  );

  @override
  void initState() {
    super.initState();
    init(
        context.read<bridge.Config>(),
        context.read<OrderChangeNotifier>(),
        context.read<PositionChangeNotifier>(),
        context.read<WalletChangeNotifier>(),
        context.read<CandlestickChangeNotifier>(),
        context.read<TradeValuesChangeNotifier>());
  }

  @override
  Widget build(BuildContext context) {
    MaterialColor swatch = Colors.blue;

    return MaterialApp.router(
      title: "10101",
      theme: ThemeData(
        primarySwatch: swatch,
        extensions: <ThemeExtension<dynamic>>[
          const TradeTheme(),
          WalletTheme(colors: ColorScheme.fromSwatch(primarySwatch: swatch)),
        ],
      ),
      routerConfig: _router,
      debugShowCheckedModeBanner: false,
    );
  }

  Future<void> init(
      bridge.Config config,
      OrderChangeNotifier orderChangeNotifier,
      PositionChangeNotifier positionChangeNotifier,
      WalletChangeNotifier walletChangeNotifier,
      CandlestickChangeNotifier candlestickChangeNotifier,
      TradeValuesChangeNotifier tradeValuesChangeNotifier) async {
    try {
      setupRustLogging();

      // TODO: Move this code into an "InitService" or similar; we should not have bridge code in the widget

      final EventService eventService = EventService.create();
      eventService.subscribe(
          orderChangeNotifier, bridge.Event.orderUpdateNotification(Order.apiDummy()));

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
          positionChangeNotifier, bridge.Event.priceUpdateNotification(Price.apiDummy()));

      eventService.subscribe(
          AnonSubscriber((event) => FLog.info(text: event.field0)), const bridge.Event.log(""));

      final appSupportDir = await getApplicationSupportDirectory();
      FLog.info(text: "App data will be stored in: $appSupportDir");

      await rust.api.run(config: config, appDir: appSupportDir.path);

      await orderChangeNotifier.initialize();
      await positionChangeNotifier.initialize();
      await candlestickChangeNotifier.initialize();

      var lastLogin = await rust.api.updateLastLogin();
      FLog.debug(text: "Last login was at ${lastLogin.date}");

      await walletChangeNotifier.refreshWalletInfo();
    } on FfiException catch (error) {
      FLog.error(text: "Failed to initialise: Error: ${error.message}", exception: error);
    } catch (error) {
      FLog.error(text: "Failed to initialise: $error", exception: error);
    } finally {
      FlutterNativeSplash.remove();
    }
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
    if (location.startsWith(WalletScreen.route)) {
      return 0;
    }
    if (location.startsWith(TradeScreen.route)) {
      return 1;
    }
    return 0;
  }

  void _onItemTapped(int index, BuildContext context) {
    switch (index) {
      case 0:
        GoRouter.of(context).go(WalletScreen.route);
        break;
      case 1:
        GoRouter.of(context).go(TradeScreen.route);
        break;
    }
  }
}
