import 'package:flutter/material.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/features/trade/application/trade_values_service.dart';
import 'package:get_10101/features/trade/domain/direction.dart';
import 'package:get_10101/features/trade/domain/leverage.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/common/application/event_service.dart';
import 'package:get_10101/common/dummy_values.dart';

import 'domain/price.dart';
import 'domain/trade_values.dart';

class TradeValuesChangeNotifier extends ChangeNotifier implements Subscriber {
  final TradeValuesService tradeValuesService;

  // The trade values are represented as Order domain, because that's essentially what they are
  late final TradeValues _buyTradeValues;
  late final TradeValues _sellTradeValues;

  late final int _feeReserve;
  late final int _channelReserve;
  late final int _minimumTradeMargin;

  TradeValuesChangeNotifier(this.tradeValuesService) {
    _buyTradeValues = _initOrder(Direction.long);
    _sellTradeValues = _initOrder(Direction.short);

    _feeReserve = tradeValuesService.getFeeReserve();
    _channelReserve = tradeValuesService.getChannelReserve();
    _minimumTradeMargin = tradeValuesService.getMinTradeMargin();
  }

  TradeValues _initOrder(Direction direction) {
    double defaultQuantity = 100;
    double defaultLeverage = 2;

    switch (direction) {
      case Direction.long:
        return TradeValues.create(
            quantity: defaultQuantity,
            leverage: Leverage(defaultLeverage),
            price: dummyAskPrice,
            fundingRate: fundingRateBuy,
            direction: direction,
            tradeValuesService: tradeValuesService);
      case Direction.short:
        return TradeValues.create(
            quantity: defaultQuantity,
            leverage: Leverage(defaultLeverage),
            price: dummyBidPrice,
            fundingRate: fundingRateSell,
            direction: direction,
            tradeValuesService: tradeValuesService);
    }
  }

  int get minMargin => _minimumTradeMargin;
  int get reserve => _feeReserve + _channelReserve;

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

  // Orderbook price updates both directions
  void updatePrice(Price price) {
    _buyTradeValues.updatePrice(price.ask);
    _sellTradeValues.updatePrice(price.bid);
    notifyListeners();
  }

  TradeValues fromDirection(Direction direction) =>
      direction == Direction.long ? _buyTradeValues : _sellTradeValues;

  @override
  void notify(bridge.Event event) {
    if (event is bridge.Event_PriceUpdateNotification) {
      updatePrice(Price.fromApi(event.field0));
    }
  }
}
