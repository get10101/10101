import 'dart:async';

import 'package:candlesticks/candlesticks.dart';
import 'package:flutter/material.dart';
import 'package:get_10101/features/trade/application/candlestick_service.dart';

class CandlestickChangeNotifier extends ChangeNotifier {
  late List<Candle> candles = [];

  final CandlestickService _candlestickService;
  Timer? timer;

  CandlestickChangeNotifier(
    this._candlestickService,
  );

  CandlestickChangeNotifier initialize() {
    _candlestickService.fetchCandles(1000).then((candles) {
      this.candles = candles;
      notifyListeners();
    });

    timer = Timer.periodic(const Duration(seconds: 30), (Timer t) async {
      final list = await _candlestickService.fetchCandles(1);
      if (list.isNotEmpty) {
        // we expect only one item to be in the list
        var item = list[0];
        if (candles.isEmpty || candles[0].date.isBefore(item.date)) {
          candles.insert(0, item);
          notifyListeners();
        }
      }
    });

    return this;
  }

  @override
  void dispose() {
    timer!.cancel();
    super.dispose();
  }
}
