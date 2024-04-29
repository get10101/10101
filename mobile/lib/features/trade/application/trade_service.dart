import 'package:get_10101/features/trade/domain/trade.dart';
import 'package:get_10101/ffi.dart' as rust;

class TradeService {
  Future<List<Trade>> fetchTrades() async {
    List<rust.Trade> apiTrades = await rust.api.getTrades();
    List<Trade> trades = apiTrades.map((trade) => Trade.fromApi(trade)).toList();

    return trades;
  }
}
