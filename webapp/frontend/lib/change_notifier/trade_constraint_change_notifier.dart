import 'package:flutter/material.dart';
import 'package:get_10101/logger/logger.dart';
import 'package:get_10101/services/trade_constraints_service.dart';

class TradeConstraintsChangeNotifier extends ChangeNotifier {
  final TradeConstraintsService service;

  TradeConstraints? _tradeConstraints;

  TradeConstraintsChangeNotifier(this.service) {
    _refresh();
  }

  void _refresh() async {
    try {
      final tradeConstraints = await service.getTradeConstraints();
      _tradeConstraints = tradeConstraints;
      super.notifyListeners();
    } catch (error) {
      logger.e(error);
    }
  }

  TradeConstraints? get tradeConstraints => _tradeConstraints;
}
