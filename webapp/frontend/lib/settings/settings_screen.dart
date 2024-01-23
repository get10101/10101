import 'package:flutter/material.dart';
import 'package:font_awesome_flutter/font_awesome_flutter.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/settings/app_info_screen.dart';
import 'package:get_10101/settings/seed_screen.dart';

class SettingsScreen extends StatefulWidget {
  static const route = "/settings";

  const SettingsScreen({super.key});

  @override
  State<SettingsScreen> createState() => _SettingsScreenState();
}

class _SettingsScreenState extends State<SettingsScreen> with SingleTickerProviderStateMixin {
  late final _tabController = TabController(length: 3, vsync: this);

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      width: 500,
      child: Column(
        mainAxisAlignment: MainAxisAlignment.center,
        crossAxisAlignment: CrossAxisAlignment.center,
        children: [
          TabBar(
            unselectedLabelColor: Colors.black,
            labelColor: tenTenOnePurple,
            tabs: const [
              Tab(
                  child: Row(
                mainAxisAlignment: MainAxisAlignment.center,
                children: [
                  Icon(FontAwesomeIcons.info, size: 20),
                  SizedBox(width: 10),
                  Text("App Info")
                ],
              )),
              Tab(
                child: Row(
                  mainAxisAlignment: MainAxisAlignment.center,
                  children: [
                    Icon(Icons.balance_outlined, size: 20),
                    SizedBox(width: 10),
                    Text("Channel")
                  ],
                ),
              ),
              Tab(
                child: Row(
                  mainAxisAlignment: MainAxisAlignment.center,
                  children: [
                    Icon(FontAwesomeIcons.seedling, size: 20),
                    SizedBox(width: 10),
                    Text("Backup")
                  ],
                ),
              ),
            ],
            controller: _tabController,
            indicatorSize: TabBarIndicatorSize.tab,
          ),
          Expanded(
            child: TabBarView(
              controller: _tabController,
              children: const [AppInfoScreen(), Text("Channel"), SeedScreen()],
            ),
          ),
        ],
      ),
    );
  }
}
