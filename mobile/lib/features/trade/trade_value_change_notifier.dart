import 'package:flutter/material.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/common/application/channel_info_service.dart';
import 'package:get_10101/common/application/event_service.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/common/dummy_values.dart';
import 'package:get_10101/features/trade/application/trade_values_service.dart';
import 'package:get_10101/features/trade/domain/direction.dart';
import 'package:get_10101/features/trade/domain/leverage.dart';
import 'package:get_10101/features/trade/domain/price.dart';
import 'package:get_10101/features/trade/domain/trade_values.dart';

class TradeValuesChangeNotifier extends ChangeNotifier implements Subscriber {
  final TradeValuesService tradeValuesService;
  final ChannelInfoService channelInfoService;

  // The trade values are represented as Order domain, because that's essentially what they are
  late final TradeValues _buyTradeValues;
  late final TradeValues _sellTradeValues;

  TradeValuesChangeNotifier(this.tradeValuesService, this.channelInfoService) {
    _buyTradeValues = _initOrder(Direction.long);
    _sellTradeValues = _initOrder(Direction.short);
  }

  TradeValues _initOrder(Direction direction) {
    Amount defaultMargin = channelInfoService.getMinTradeMargin();
    Leverage defaultLeverage = Leverage(2);

    switch (direction) {
      case Direction.long:
        return TradeValues.fromMargin(
            margin: defaultMargin,
            leverage: defaultLeverage,
            price: null,
            fundingRate: fundingRateBuy,
            direction: direction,
            tradeValuesService: tradeValuesService);
      case Direction.short:
        return TradeValues.fromMargin(
            margin: defaultMargin,
            leverage: defaultLeverage,
            price: null,
            fundingRate: fundingRateSell,
            direction: direction,
            tradeValuesService: tradeValuesService);
    }
  }

  /// Calculates the counterparty margin based on leverage one
  int? counterpartyMargin(Direction direction, double leverage) {
    switch (direction) {
      case Direction.long:
        return tradeValuesService
            .calculateMargin(
                price: _buyTradeValues.price,
                quantity: _buyTradeValues.quantity,
                leverage: Leverage(leverage))
            ?.sats;
      case Direction.short:
        return tradeValuesService
            .calculateMargin(
                price: _sellTradeValues.price,
                quantity: _sellTradeValues.quantity,
                leverage: Leverage(leverage))
            ?.sats;
    }
  }

  Amount? orderMatchingFee(Direction direction) {
    return fromDirection(direction).fee;
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

  // Orderbook price updates both directions
  void updatePrice(Price price) {
    bool update = false;

    if (price.ask != _buyTradeValues.price) {
      _buyTradeValues.updatePriceAndQuantity(price.ask);
      update = true;
    }
    if (price.bid != _sellTradeValues.price) {
      _sellTradeValues.updatePriceAndQuantity(price.bid);
      update = true;
    }

    if (update) {
      notifyListeners();
    }
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
