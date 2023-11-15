import 'package:flutter/material.dart';
import 'package:flutter_native_splash/flutter_native_splash.dart';
import 'package:get_10101/backend.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/common/color.dart';
import 'package:get_10101/util/compare_coordinator_version.dart';
import 'package:get_10101/common/init_service.dart';
import 'package:get_10101/common/routes.dart';
import 'package:get_10101/features/trade/trade_theme.dart';
import 'package:get_10101/features/wallet/wallet_theme.dart';
import 'package:get_10101/util/notifications.dart';
import 'package:go_router/go_router.dart';
import 'package:provider/provider.dart';
import 'package:get_10101/logger/logger.dart';

void main() async {
  WidgetsBinding widgetsBinding = WidgetsFlutterBinding.ensureInitialized();
  FlutterNativeSplash.preserve(widgetsBinding: widgetsBinding);

  await initFirebase();
  await setConfig();

  runApp(MultiProvider(providers: createProviders(), child: const TenTenOneApp()));
}

class TenTenOneApp extends StatefulWidget {
  const TenTenOneApp({Key? key}) : super(key: key);

  @override
  State<TenTenOneApp> createState() => _TenTenOneAppState();
}

class _TenTenOneAppState extends State<TenTenOneApp> with WidgetsBindingObserver {
  final GlobalKey<ScaffoldMessengerState> scaffoldMessengerKey =
      GlobalKey<ScaffoldMessengerState>();

  late GoRouter _router;

  @override
  void initState() {
    super.initState();

    WidgetsBinding.instance.addObserver(this);

    final config = context.read<bridge.Config>();
    _router = createRoutes();

    subscribeToNotifiers(context);

    // TODO(holzeis): check if we can do this without the addPostFrameCallback
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
}
