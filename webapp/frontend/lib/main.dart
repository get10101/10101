import 'package:flutter/material.dart';
import 'package:get_10101/common/version_service.dart';
import 'package:get_10101/logger/logger.dart';
import 'package:get_10101/routes.dart';
import 'package:get_10101/settings/settings_service.dart';
import 'package:get_10101/wallet/wallet_service.dart';
import 'package:provider/provider.dart';

import 'common/color.dart';
import 'common/theme.dart';

void main() {
  WidgetsFlutterBinding.ensureInitialized();
  buildLogger(false);
  logger.i("Logger initialized");

  var providers = [
    Provider(create: (context) => const VersionService()),
    Provider(create: (context) => const WalletService()),
    Provider(create: (context) => const SettingsService())
  ];
  runApp(MultiProvider(providers: providers, child: const TenTenOneApp()));
}

class TenTenOneApp extends StatefulWidget {
  const TenTenOneApp({super.key});

  @override
  State<TenTenOneApp> createState() => _TenTenOneAppState();
}

class _TenTenOneAppState extends State<TenTenOneApp> {
  final GlobalKey<ScaffoldMessengerState> scaffoldMessengerKey =
      GlobalKey<ScaffoldMessengerState>();

  @override
  void initState() {
    super.initState();
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
        extensions: const <ThemeExtension<dynamic>>[
          TenTenOneTheme(),
        ],
      ),
      routerConfig: goRouter,
      debugShowCheckedModeBanner: false,
    );
  }
}
