import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/features/trade/domain/direction.dart';
import 'package:get_10101/features/trade/domain/contract_symbol.dart';

class Trade implements Comparable<Trade> {
  final TradeType tradeType;
  final ContractSymbol contractSymbol;
  final Direction direction;
  final Usd quantity;
  final Usd price;
  final Amount fee;
  Amount? pnl;
  final DateTime timestamp;
  final bool isDone;

  Trade({
    required this.tradeType,
    required this.contractSymbol,
    required this.direction,
    required this.quantity,
    required this.price,
    required this.fee,
    this.pnl,
    required this.timestamp,
    required this.isDone,
  });

  @override
  int compareTo(Trade other) {
    int comp = other.timestamp.compareTo(timestamp);

    // Sometimes two trades might have the same timestamp. This can happen
    // when we change position direction. In that case, we want the trade that
    // first reduces the position to zero to appear first.
    if (comp == 0) {
      if (pnl != null) {
        return 1;
      } else {
        return -1;
      }
    }

    return comp;
  }

  static Trade fromApi(bridge.Trade trade) {
    return Trade(
        tradeType: TradeType.fromApi(trade.tradeType),
        contractSymbol: ContractSymbol.fromApi(trade.contractSymbol),
        direction: Direction.fromApi(trade.direction),
        quantity: Usd.fromDouble(trade.contracts),
        price: Usd.fromDouble(trade.price),
        // Positive fees coming from Rust are paid by the trader. We flip the sign here, because
        // that is how we want to display them.
        fee: Amount(-trade.fee),
        timestamp: DateTime.fromMillisecondsSinceEpoch(trade.timestamp * 1000),
        pnl: trade.pnl != null ? Amount(trade.pnl!) : null,
        isDone: trade.isDone);
  }

  static bridge.Trade apiDummy() {
    return const bridge.Trade(
      tradeType: bridge.TradeType.Trade,
      contractSymbol: bridge.ContractSymbol.BtcUsd,
      contracts: 0,
      price: 0,
      fee: 0,
      direction: bridge.Direction.Long,
      timestamp: 0,
      isDone: true,
    );
  }
}

enum TradeType {
  trade,
  funding;

  static TradeType fromApi(bridge.TradeType tradeType) {
    switch (tradeType) {
      case bridge.TradeType.Trade:
        return TradeType.trade;
      case bridge.TradeType.Funding:
        return TradeType.funding;
    }
  }
}
