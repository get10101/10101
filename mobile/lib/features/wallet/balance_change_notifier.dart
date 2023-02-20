import 'dart:developer';

import 'package:flutter/material.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart';

class BalanceChangeNotifier extends ChangeNotifier {
  Balance balance = Balance(onChain: 0, offChain: 0);

  void update(Balance balance) {
    this.balance = balance;
    log("BalanceChangeNotifier: ${this.balance.onChain}");
    super.notifyListeners();
  }
}
