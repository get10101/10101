import 'dart:async';

import 'package:decimal/decimal.dart';
import 'package:flutter/material.dart';
import 'package:font_awesome_flutter/font_awesome_flutter.dart';
import 'package:get_10101/change_notifier/quote_change_notifier.dart';
import 'package:get_10101/services/auth_service.dart';
import 'package:get_10101/auth/login_screen.dart';
import 'package:get_10101/common/amount_text.dart';
import 'package:get_10101/common/balance.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/change_notifier/currency_change_notifier.dart';
import 'package:get_10101/common/currency_selection_widget.dart';
import 'package:get_10101/common/model.dart';
import 'package:get_10101/common/snack_bar.dart';
import 'package:get_10101/services/version_service.dart';
import 'package:get_10101/logger/logger.dart';
import 'package:get_10101/services/quote_service.dart';
import 'package:get_10101/change_notifier/wallet_change_notifier.dart';
import 'package:go_router/go_router.dart';
import 'package:provider/provider.dart';

class NavigationDestinations {
  const NavigationDestinations(this.label, this.icon, this.selectedIcon);

  final String label;
  final Widget icon;
  final Widget selectedIcon;
}

const List<NavigationDestinations> destinations = <NavigationDestinations>[
  NavigationDestinations('Trading', Icon(Icons.bar_chart_outlined), Icon(Icons.bar_chart)),
  NavigationDestinations('Wallet', Icon(Icons.wallet_outlined), Icon(Icons.wallet)),
  NavigationDestinations('Settings', Icon(Icons.settings_outlined), Icon(Icons.settings)),
];

class ScaffoldWithNestedNavigation extends StatefulWidget {
  const ScaffoldWithNestedNavigation({
    Key? key,
    required this.navigationShell,
  }) : super(key: key ?? const ValueKey<String>('ScaffoldWithNestedNavigation'));
  final StatefulNavigationShell navigationShell;

  @override
  State<ScaffoldWithNestedNavigation> createState() => _ScaffoldWithNestedNavigation();
}

// Based on
// https://github.com/flutter/packages/blob/main/packages/go_router/example/lib/stateful_shell_route.dart
class _ScaffoldWithNestedNavigation extends State<ScaffoldWithNestedNavigation> {
  late bool showNavigationDrawer;
  late bool showAsDrawer;

  String version = "unknown";
  Timer? _timeout;

  // sets the timeout until the user will get automatically logged out after inactivity.
  final _inactivityTimout = const Duration(minutes: 5);

  void _goBranch(int index) {
    widget.navigationShell.goBranch(
      index,
      initialLocation: index == widget.navigationShell.currentIndex,
    );
  }

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    showNavigationDrawer = MediaQuery.of(context).size.width >= 450;
    showAsDrawer = MediaQuery.of(context).size.width >= 1024;
  }

  @override
  void initState() {
    super.initState();
    context.read<VersionService>().fetchVersion().then((v) => setState(() => version = v.version));
  }

  @override
  void dispose() {
    super.dispose();
    _timeout?.cancel();
  }

  @override
  Widget build(BuildContext context) {
    final navigationShell = widget.navigationShell;

    final walletChangeNotifier = context.watch<WalletChangeNotifier>();
    final quoteChangeNotifier = context.watch<QuoteChangeNotifier>();

    final authService = context.read<AuthService>();

    if (_timeout != null) _timeout!.cancel();
    _timeout = Timer(_inactivityTimout, () {
      logger.i("Signing out due to inactivity");
      authService.signOut();
      GoRouter.of(context).go(LoginScreen.route);
    });

    if (showNavigationDrawer) {
      return ScaffoldWithNavigationRail(
        body: navigationShell,
        selectedIndex: navigationShell.currentIndex,
        onDestinationSelected: _goBranch,
        showAsDrawer: showAsDrawer,
        version: version,
        balance: walletChangeNotifier.getBalance(),
        bestQuote: quoteChangeNotifier.getBestQuote(),
      );
    } else {
      return ScaffoldWithNavigationBar(
        body: navigationShell,
        selectedIndex: navigationShell.currentIndex,
        onDestinationSelected: _goBranch,
      );
    }
  }
}

class ScaffoldWithNavigationBar extends StatelessWidget {
  const ScaffoldWithNavigationBar({
    super.key,
    required this.body,
    required this.selectedIndex,
    required this.onDestinationSelected,
  });

  final Widget body;
  final int selectedIndex;
  final ValueChanged<int> onDestinationSelected;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: body,
      bottomNavigationBar: NavigationBar(
        overlayColor: WidgetStateColor.resolveWith(
          (Set<WidgetState> states) {
            if (states.contains(WidgetState.focused)) {
              return Colors.green;
            }
            if (states.contains(WidgetState.error)) {
              return Colors.red;
            }
            return Colors.grey;
          },
        ),
        selectedIndex: selectedIndex,
        destinations: const [
          NavigationDestination(label: 'Trading', icon: Icon(Icons.bar_chart)),
          NavigationDestination(label: 'Wallet', icon: Icon(Icons.wallet)),
          NavigationDestination(label: 'Settings', icon: Icon(Icons.settings)),
        ],
        onDestinationSelected: onDestinationSelected,
      ),
    );
  }
}

class ScaffoldWithNavigationRail extends StatelessWidget {
  const ScaffoldWithNavigationRail({
    super.key,
    required this.body,
    required this.selectedIndex,
    required this.onDestinationSelected,
    required this.showAsDrawer,
    required this.version,
    required this.balance,
    required this.bestQuote,
  });

  final Widget body;
  final int selectedIndex;
  final ValueChanged<int> onDestinationSelected;
  final bool showAsDrawer;
  final String version;
  final Balance? balance;
  final BestQuote? bestQuote;

  @override
  Widget build(BuildContext context) {
    final quoteChangeNotifier = context.watch<QuoteChangeNotifier>();
    final quote = quoteChangeNotifier.getBestQuote();
    final Price midMarket =
        ((quote?.ask ?? Price.zero()) + (quote?.bid ?? Price.zero())) / Decimal.fromInt(2);

    final currencyChangeNotifier = context.watch<CurrencyChangeNotifier>();
    final currency = currencyChangeNotifier.currency;

    return Scaffold(
      body: Row(
        children: [
          NavigationRail(
            extended: showAsDrawer,
            selectedIndex: selectedIndex,
            onDestinationSelected: onDestinationSelected,
            trailing: Expanded(
              child: Column(
                mainAxisAlignment: MainAxisAlignment.end,
                children: [
                  Row(
                    children: [
                      Text("v$version"),
                    ],
                  ),
                  const SizedBox(height: 10),
                  const Row(
                    children: [CurrencySelectionScreen()],
                  ),
                  const SizedBox(height: 20),
                ],
              ),
            ),
            leading: showAsDrawer
                ? Image.asset("assets/10101_flat_logo.png", width: 200, height: 50)
                : Image.asset("assets/10101_logo_icon.png", width: 50, height: 50),
            labelType: showAsDrawer ? NavigationRailLabelType.none : NavigationRailLabelType.all,
            destinations: destinations
                .map(
                  (navigation) => NavigationRailDestination(
                      label: Text(navigation.label),
                      icon: navigation.icon,
                      selectedIcon: navigation.selectedIcon),
                )
                .toList(),
          ),
          const VerticalDivider(thickness: 1, width: 1),
          // This is the main content.
          Expanded(
            child: Column(
              children: [
                Container(
                  decoration: const BoxDecoration(
                      border: Border(bottom: BorderSide(width: 0.5, color: Colors.grey))),
                  padding: const EdgeInsets.all(25),
                  child: Row(
                    children: [
                      Expanded(
                        child: SingleChildScrollView(
                          scrollDirection: Axis.horizontal,
                          child: Row(
                            children: [
                              TopBarItem(
                                  label: 'Latest Bid: ',
                                  value: bestQuote?.bid == null
                                      ? []
                                      : [
                                          TextSpan(
                                            text: bestQuote?.bid?.toString(),
                                            style: const TextStyle(fontWeight: FontWeight.bold),
                                          )
                                        ]),
                              const SizedBox(width: 30),
                              TopBarItem(
                                  label: 'Latest Ask: ',
                                  value: bestQuote?.ask == null
                                      ? []
                                      : [
                                          TextSpan(
                                            text: bestQuote?.ask?.toString(),
                                            style: const TextStyle(fontWeight: FontWeight.bold),
                                          )
                                        ]),
                              const SizedBox(width: 30),
                              TopBarItem(
                                  label: 'Off-chain: ',
                                  value: balance == null
                                      ? []
                                      : [
                                          formatAmountAsCurrency(
                                              balance?.offChain, currency, midMarket),
                                        ]),
                              const SizedBox(width: 30),
                              TopBarItem(
                                  label: 'On-chain: ',
                                  value: balance == null
                                      ? []
                                      : [
                                          formatAmountAsCurrency(
                                              balance?.onChain, currency, midMarket),
                                        ]),
                              const SizedBox(width: 30),
                              TopBarItem(
                                  label: 'Total: ',
                                  value: balance == null
                                      ? []
                                      : [
                                          formatAmountAsCurrency(
                                              (balance?.onChain ?? Amount.zero()) +
                                                  (balance?.offChain ?? Amount.zero()),
                                              currency,
                                              midMarket),
                                        ]),
                            ],
                          ),
                        ),
                      ),
                      const SizedBox(width: 10),
                      IconButton(
                        onPressed: () {
                          context
                              .read<AuthService>()
                              .signOut()
                              .then((value) => GoRouter.of(context).go(LoginScreen.route))
                              .catchError((error) {
                            final messenger = ScaffoldMessenger.of(context);
                            showSnackBar(messenger, error);
                          });
                        },
                        icon: const CircleAvatar(
                          backgroundColor: tenTenOnePurple,
                          child: Icon(
                            FontAwesomeIcons.arrowRightFromBracket,
                            color: Colors.white,
                            size: 14,
                          ),
                        ),
                        color: Colors.white,
                        iconSize: 16,
                        padding: const EdgeInsets.all(4),
                        splashRadius: 15,
                        constraints: const BoxConstraints(),
                      )
                    ],
                  ),
                ),
                Expanded(
                  child: body,
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

class TopBarItem extends StatelessWidget {
  final String label;
  final List<InlineSpan> value;

  const TopBarItem({super.key, required this.label, required this.value});

  @override
  Widget build(BuildContext context) {
    return value.isEmpty
        ? Row(
            mainAxisAlignment: MainAxisAlignment.center,
            children: <Widget>[
              Text(label, style: const TextStyle(color: Colors.black)),
              const SizedBox(width: 10),
              const SizedBox(
                width: 20,
                height: 20,
                child: CircularProgressIndicator(),
              ),
            ],
          )
        : RichText(
            text: TextSpan(
              text: label,
              style: const TextStyle(fontSize: 16, color: Colors.black),
              children: value,
            ),
          );
  }
}

TextSpan formatAmountAsCurrency(Amount? amount, Currency currency, Price midMarket) {
  if (amount == null) {
    return const TextSpan();
  }

  String formatted = "";
  switch (currency) {
    case Currency.usd:
      formatted = formatUsd(amount * midMarket, decimalPlaces: 2);
    case Currency.btc:
      formatted = formatBtc(amount);
    case Currency.sats:
      formatted = formatSats(amount);
  }

  return TextSpan(text: formatted, style: const TextStyle(fontWeight: FontWeight.bold));
}
