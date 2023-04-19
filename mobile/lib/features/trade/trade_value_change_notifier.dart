import 'package:flutter/material.dart';
import 'package:get_10101/common/application/channel_constraints_service.dart';
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
  final ChannelConstraintsService _channelConstraintsService;

  // The trade values are represented as Order domain, because that's essentially what they are
  late final TradeValues _buyTradeValues;
  late final TradeValues _sellTradeValues;

  late final int _feeReserve;
  late final int _channelReserve;
  late final int _minimumTradeMargin;
  late final int _channelCapacity;

  TradeValuesChangeNotifier(this.tradeValuesService, this._channelConstraintsService) {
    _buyTradeValues = _initOrder(Direction.long);
    _sellTradeValues = _initOrder(Direction.short);

    _feeReserve = _channelConstraintsService.getFeeReserve();
    _channelReserve = _channelConstraintsService.getChannelReserve();
    _minimumTradeMargin = _channelConstraintsService.getMinTradeMargin();
    _channelCapacity = _channelConstraintsService.getLightningChannelCapacity();
  }

  TradeValues _initOrder(Direction direction) {
    Amount defaultMargin = Amount(_channelConstraintsService.getMinTradeMargin());
    Leverage defaultLeverage = Leverage(2);

    switch (direction) {
      case Direction.long:
        return TradeValues.create(
            margin: defaultMargin,
            leverage: defaultLeverage,
            price: dummyAskPrice,
            fundingRate: fundingRateBuy,
            direction: direction,
            tradeValuesService: tradeValuesService);
      case Direction.short:
        return TradeValues.create(
            margin: defaultMargin,
            leverage: defaultLeverage,
            price: dummyBidPrice,
            fundingRate: fundingRateSell,
            direction: direction,
            tradeValuesService: tradeValuesService);
    }
  }

  int get minMargin => _minimumTradeMargin;
  int get reserve => _feeReserve + _channelReserve;
  int get channelReserve => _channelReserve;
  int get feeReserve => _feeReserve;
  int get capacity => _channelCapacity;

  /// Calculates the counterparty margin based on leverage one
  int counterpartyMargin(Direction direction) {
    switch (direction) {
      case Direction.long:
        return tradeValuesService
            .calculateMargin(
                price: _buyTradeValues.price,
                quantity: _buyTradeValues.quantity,
                leverage: Leverage(1))
            .sats;
      case Direction.short:
        return tradeValuesService
            .calculateMargin(
                price: _sellTradeValues.price,
                quantity: _sellTradeValues.quantity,
                leverage: Leverage(1))
            .sats;
    }
  }

  /// Defines the amount of sats the user can actually use for trading
  /// Defined as:
  /// available_trading_capacity = channel_capacity - total_reserve - counterparty_margin
  int availableTradingCapacity(Direction direction) {
    int channelCapacity = _channelConstraintsService.getLightningChannelCapacity();
    int totalReserve = reserve * 2;

    return channelCapacity - totalReserve - counterpartyMargin(direction);
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
      _buyTradeValues.updatePrice(price.ask);
      update = true;
    }
    if (price.bid != _sellTradeValues.price) {
      _sellTradeValues.updatePrice(price.bid);
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
