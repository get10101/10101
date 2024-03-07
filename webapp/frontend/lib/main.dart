import 'package:flutter/material.dart';
import 'package:get_10101/auth/auth_service.dart';
import 'package:get_10101/common/currency_change_notifier.dart';
import 'package:get_10101/common/version_service.dart';
import 'package:get_10101/logger/logger.dart';
import 'package:get_10101/routes.dart';
import 'package:get_10101/settings/channel_change_notifier.dart';
import 'package:get_10101/settings/channel_service.dart';
import 'package:get_10101/trade/order_change_notifier.dart';
import 'package:get_10101/trade/order_service.dart';
import 'package:get_10101/trade/position_change_notifier.dart';
import 'package:get_10101/trade/position_service.dart';
import 'package:get_10101/trade/quote_change_notifier.dart';
import 'package:get_10101/trade/quote_service.dart';
import 'package:get_10101/settings/settings_service.dart';
import 'package:get_10101/wallet/wallet_change_notifier.dart';
import 'package:get_10101/wallet/wallet_service.dart';
import 'package:intl/date_symbol_data_local.dart';
import 'package:intl/intl_browser.dart';
import 'package:provider/provider.dart';

import 'common/color.dart';
import 'common/theme.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  buildLogger(false);
  logger.i("Logger initialized");

  // Get the system's default locale
  String defaultLocale = await findSystemLocale();

  // Initialize the date format data for the system's default locale
  await initializeDateFormatting(defaultLocale, null);

  const walletService = WalletService();
  const channelService = ChannelService();

  var providers = [
    Provider(create: (context) => const VersionService()),
    ChangeNotifierProvider(create: (context) => WalletChangeNotifier(walletService)),
    ChangeNotifierProvider(create: (context) => QuoteChangeNotifier(const QuoteService())),
    ChangeNotifierProvider(create: (context) => PositionChangeNotifier(const PositionService())),
    ChangeNotifierProvider(create: (context) => OrderChangeNotifier(const OrderService())),
    ChangeNotifierProvider(create: (context) => ChannelChangeNotifier(channelService)),
    ChangeNotifierProvider(create: (context) => CurrencyChangeNotifier(Currency.sats)),
    Provider(create: (context) => const SettingsService()),
    Provider(create: (context) => channelService),
    Provider(create: (context) => AuthService()),
    Provider(create: (context) => walletService)
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
  Widget build(BuildContext context) {
    MaterialColor swatch = tenTenOnePurple;
    final ColorScheme customColorScheme =
        ColorScheme.fromSwatch(backgroundColor: Colors.grey[50], primarySwatch: tenTenOnePurple);

    return MaterialApp.router(
      title: "10101",
      scaffoldMessengerKey: scaffoldMessengerKey,
      supportedLocales: const [
        Locale('en', 'US'),
        Locale('es', 'ES'),
        Locale('fr', 'FR'),
        Locale('de', 'DE'),
      ],
      theme: ThemeData(
        primarySwatch: swatch,
        bottomNavigationBarTheme: const BottomNavigationBarThemeData(
          selectedLabelStyle: TextStyle(color: tenTenOnePurple),
        ),
        navigationRailTheme: const NavigationRailThemeData(
          selectedLabelTextStyle: TextStyle(
            color: tenTenOnePurple,
          ),
        ),
        inputDecorationTheme: InputDecorationTheme(
          prefixIconColor: MaterialStateColor.resolveWith(
            (Set<MaterialState> states) {
              if (states.contains(MaterialState.focused)) {
                return tenTenOnePurple;
              }
              if (states.contains(MaterialState.error)) {
                return Colors.red;
              }
              return Colors.grey;
            },
          ),
        ),
        elevatedButtonTheme: ElevatedButtonThemeData(
          style: ButtonStyle(
            // this is the button background color
            backgroundColor: MaterialStateProperty.resolveWith<Color>(
              (Set<MaterialState> states) {
                if (states.contains(MaterialState.disabled)) {
                  // Return grey color when the button is disabled
                  return Colors.grey;
                }
                // Return your default color when button is enabled
                return tenTenOnePurple;
              },
            ),
            // this is the button text color
            foregroundColor: MaterialStateProperty.all<Color>(Colors.white),
            shape: MaterialStateProperty.all<RoundedRectangleBorder>(
              RoundedRectangleBorder(
                borderRadius: BorderRadius.circular(5.0),
              ),
            ),
          ),
        ),
        colorScheme: customColorScheme,
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
