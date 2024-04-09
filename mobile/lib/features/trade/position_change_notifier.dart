import 'package:get_10101/features/trade/domain/direction.dart';
import 'package:get_10101/features/trade/domain/leverage.dart';
import 'package:get_10101/logger/logger.dart';
import 'package:flutter/material.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/common/application/event_service.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/features/trade/application/position_service.dart';
import 'package:get_10101/features/trade/domain/contract_symbol.dart';
import 'package:get_10101/util/preferences.dart';
import 'package:get_10101/features/trade/domain/position.dart';

class PositionChangeNotifier extends ChangeNotifier implements Subscriber {
  final PositionService _positionService;

  Map<ContractSymbol, Position> positions = {};

  double? askPrice;
  double? bidPrice;

  /// Amount of stabilised bitcoin in terms of USD (fiat)
  double getStableUSDAmountInFiat() {
    if (hasStableUSD()) {
      final positionUsd = positions[ContractSymbol.btcusd];
      return positionUsd!.quantity.asDouble();
    } else {
      return 0.0;
    }
  }

  Amount getStableUSDAmountInSats() {
    if (hasStableUSD()) {
      final positionUsd = positions[ContractSymbol.btcusd];
      return positionUsd!.getAmountWithUnrealizedPnl();
    } else {
      return Amount(0);
    }
  }

  bool hasStableUSD() {
    final positionUsd = positions[ContractSymbol.btcusd];
    return positionUsd != null && positionUsd.stable;
  }

  Amount marginUsableForTrade(Direction tradeDirection) {
    final position = positions[ContractSymbol.btcusd];

    if (position == null ||
        // The margin can only be used in another trade if the trade reduces the position, by going
        // in a different direction.
        tradeDirection == position.direction ||
        position.averageEntryPrice == 0 ||
        position.leverage.leverage == 0) {
      return Amount.zero();
    }

    double marginBtc =
        position.quantity.asDouble() / (position.averageEntryPrice * position.leverage.leverage);

    return btcToSat(marginBtc);
  }

  Amount coordinatorMarginUsableForTrade(Leverage coordinatorLeverage, Direction tradeDirection) {
    final position = positions[ContractSymbol.btcusd];

    if (position == null ||
        // The coordinator margin can only be used in another trade if the trade reduces the
        // position, by going in a different direction.
        tradeDirection == position.direction ||
        position.averageEntryPrice == 0 ||
        coordinatorLeverage.leverage == 0) {
      return Amount.zero();
    }

    double marginBtc =
        position.quantity.asDouble() / (position.averageEntryPrice * coordinatorLeverage.leverage);

    return btcToSat(marginBtc);
  }

  Future<void> initialize() async {
    List<Position> positions = await _positionService.fetchPositions();
    for (Position position in positions) {
      this.positions[position.contractSymbol] = position;
    }

    notifyListeners();
  }

  PositionChangeNotifier(this._positionService);

  @override
  void notify(bridge.Event event) {
    if (event is bridge.Event_PositionUpdateNotification) {
      Position position = Position.fromApi(event.field0);

      if (askPrice != null && bidPrice != null) {
        final pnl = _positionService.calculatePnl(position, askPrice!, bidPrice!);
        position.unrealizedPnl = pnl != null ? Amount(pnl) : null;
      } else {
        position.unrealizedPnl = null;
      }
      positions[position.contractSymbol] = position;

      if (position.isStable()) {
        Preferences.instance.setOpenStablePosition();
      } else {
        Preferences.instance.setOpenTradePosition();
      }

      notifyListeners();
    } else if (event is bridge.Event_PositionClosedNotification) {
      ContractSymbol contractSymbol = ContractSymbol.fromApi(event.field0.contractSymbol);
      positions.remove(contractSymbol);

      Preferences.instance.unsetOpenPosition();

      notifyListeners();
    } else if (event is bridge.Event_AskPriceUpdateNotification ||
        event is bridge.Event_BidPriceUpdateNotification) {
      if (event is bridge.Event_AskPriceUpdateNotification) {
        askPrice = event.field0;
      }
      if (event is bridge.Event_BidPriceUpdateNotification) {
        bidPrice = event.field0;
      }

      for (ContractSymbol symbol in positions.keys) {
        if (askPrice != null && bidPrice != null) {
          if (positions[symbol] != null) {
            // TODO: we can optimize this as we know the direction already we should only need to pass in one of the prices
            final pnl = _positionService.calculatePnl(positions[symbol]!, askPrice!, bidPrice!);
            positions[symbol]!.unrealizedPnl = pnl != null ? Amount(pnl) : null;
          }
        }
      }

      notifyListeners();
    } else {
      logger.w("Received unexpected event: ${event.toString()}");
    }
  }
}

Amount btcToSat(double btc) {
  String btcString = btc.toStringAsFixed(8);

  int sats = (double.parse(btcString) * 100000000).round();

  return Amount(sats);
}
