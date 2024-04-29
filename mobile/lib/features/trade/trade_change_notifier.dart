import 'dart:collection';

import 'package:flutter/material.dart';
import 'package:get_10101/features/trade/application/trade_service.dart';
import 'package:get_10101/features/trade/domain/trade.dart';
import 'package:get_10101/logger/logger.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/common/application/event_service.dart';

class TradeChangeNotifier extends ChangeNotifier implements Subscriber {
  late TradeService _tradeService;
  Set<Trade> trades = SplayTreeSet<Trade>();

  Future<void> initialize() async {
    trades = SplayTreeSet.from(await _tradeService.fetchTrades());

    notifyListeners();
  }

  TradeChangeNotifier(TradeService tradeService) {
    _tradeService = tradeService;
  }

  @override
  void notify(bridge.Event event) {
    if (event is bridge.Event_NewTrade) {
      Trade trade = Trade.fromApi(event.field0);
      trades.add(trade);

      notifyListeners();
    } else {
      logger.w("Received unexpected event: ${event.toString()}");
    }
  }
}
