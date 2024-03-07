import 'dart:async';

import 'package:flutter/material.dart';
import 'package:get_10101/common/balance.dart';
import 'package:get_10101/common/payment.dart';
import 'package:get_10101/logger/logger.dart';
import 'package:get_10101/wallet/wallet_service.dart';

class WalletChangeNotifier extends ChangeNotifier {
  final WalletService service;
  late Timer timer;

  Balance? _balance;
  List<OnChainPayment>? _history;

  WalletChangeNotifier(this.service) {
    refresh();
    Timer.periodic(const Duration(seconds: 30), (timer) async {
      await refresh();
    });
  }

  Future<void> refresh() async {
    try {
      final data =
          await Future.wait<dynamic>([service.getBalance(), service.getOnChainPaymentHistory()]);
      _balance = data[0];
      _history = data[1];

      super.notifyListeners();
    } catch (error) {
      logger.e(error);
    }
  }

  Balance? getBalance() => _balance;

  List<OnChainPayment>? getHistory() => _history;

  @override
  void dispose() {
    super.dispose();
    timer.cancel();
  }
}
