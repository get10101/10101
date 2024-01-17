import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';

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
  Widget build(BuildContext context) {
    final navigationShell = widget.navigationShell;
    if (showNavigationDrawer) {
      return ScaffoldWithNavigationRail(
        body: navigationShell,
        selectedIndex: navigationShell.currentIndex,
        onDestinationSelected: _goBranch,
        showAsDrawer: showAsDrawer,
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
  });

  final Widget body;
  final int selectedIndex;
  final ValueChanged<int> onDestinationSelected;
  final bool showAsDrawer;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: Row(
        children: [
          NavigationRail(
            extended: showAsDrawer,
            selectedIndex: selectedIndex,
            onDestinationSelected: onDestinationSelected,
            leading: showAsDrawer
                ? Image.asset("10101_flat_logo.png", width: 200, height: 50)
                : Image.asset("10101_logo_icon.png", width: 50, height: 50),
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
            child: body,
          ),
        ],
      ),
    );
  }
}
