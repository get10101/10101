import 'package:flutter/material.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/features/trade/application/trade_values_service.dart';
import 'package:get_10101/features/trade/domain/direction.dart';
import 'package:get_10101/features/trade/domain/leverage.dart';

import 'domain/price.dart';
import 'domain/trade_values.dart';

class TradeValuesChangeNotifier extends ChangeNotifier {
  final TradeValuesService tradeValuesService;

  // The trade values are represented as Order domain, because that's essentially what they are
  final TradeValues? _buyTradeValues;
  final TradeValues? _sellTradeValues;

  // TODO: Replace dummy price with price from backend
  // TODO: Get price from separate change notifier; might be able to use a proxy change notifiers
  final Price? _currentPrice;

  // TODO replace dummy funding rate with funding rate from backend
  static const double fundingRateBuy = 0.003;
  static const double fundingRateSell = -0.003;

  TradeValuesChangeNotifier(this.tradeValuesService) {
    _buyTradeValues = _initTradeValues(Direction.long);
    _sellTradeValues = _initTradeValues(Direction.short);
  }

  TradeValues _initTradeValues(Direction direction) {
    double defaultQuantity = 100;
    double defaultLeverage = 2;

    switch (direction) {
      case Direction.long:
        return TradeValues.create(
            quantity: defaultQuantity,
            leverage: Leverage(defaultLeverage),
            price: _currentPrice.bid,
            fundingRate: fundingRateBuy,
            direction: direction,
            tradeValuesService: tradeValuesService);
      case Direction.short:
        return TradeValues.create(
            quantity: defaultQuantity,
            leverage: Leverage(defaultLeverage),
            price: _currentPrice.ask,
            fundingRate: fundingRateSell,
            direction: direction,
            tradeValuesService: tradeValuesService);
    }
  }

  void updateQuantity(Direction direction, double quantity) {
    fromDirection(direction).updateQuantity(quantity);
    notifyListeners();
  }

  void updateLeverage(Direction direction, Leverage leverage) {
    fromDirection(direction).updateLeverage(leverage);
    notifyListeners();
  }

  void updateMargin(Direction direction, Amount margin) {
    fromDirection(direction).updateMargin(margin);
    notifyListeners();
  }

  TradeValues fromDirection(Direction direction) =>
      direction == Direction.long ? _buyTradeValues : _sellTradeValues;
}
