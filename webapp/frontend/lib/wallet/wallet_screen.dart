import 'package:flutter/material.dart';
import 'package:font_awesome_flutter/font_awesome_flutter.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/wallet/receive_screen.dart';
import 'package:get_10101/wallet/send_screen.dart';

class WalletScreen extends StatefulWidget {
  static const route = "/wallet";

  const WalletScreen({super.key});

  @override
  State<WalletScreen> createState() => _WalletScreenState();
}

class _WalletScreenState extends State<WalletScreen> with SingleTickerProviderStateMixin {
  late final _tabController = TabController(length: 2, vsync: this);

  @override
  Widget build(BuildContext context) {
    return Container(
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
                    Icon(FontAwesomeIcons.arrowDown, size: 20),
                    SizedBox(width: 10),
                    Text("Receive")
                  ],
                ),
              ),
              Tab(
                child: Row(
                  mainAxisAlignment: MainAxisAlignment.center,
                  children: [
                    Icon(FontAwesomeIcons.arrowUp, size: 20),
                    SizedBox(width: 10),
                    Text("Withdraw")
                  ],
                ),
              )
            ],
            controller: _tabController,
            indicatorSize: TabBarIndicatorSize.tab,
          ),
          Expanded(
            child: TabBarView(
              controller: _tabController,
              children: const [ReceiveScreen(), SendScreen()],
            ),
          ),
        ],
      ),
    );
  }
}
