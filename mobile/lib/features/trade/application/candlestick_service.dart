import 'package:candlesticks/candlesticks.dart';
import 'package:http/http.dart' as http;
import 'dart:async';
import 'dart:convert';

class CandlestickService {
  const CandlestickService();

  Future<List<Candle>> fetchCandles(int amount) async {
    final uri = Uri.parse(
        "https://www.bitmex.com/api/v1/trade/bucketed?binSize=1m&partial=false&symbol=XBTUSD&count=$amount&reverse=true");
    final res = await http.get(uri);
    return (jsonDecode(res.body) as List<dynamic>).map((e) => _parse(e)).toList().toList();
  }

  Candle _parse(Map<String, dynamic> json) {
    var date = DateTime.parse(json['timestamp']);
    var high = json['high'].toDouble();
    var low = json['low'].toDouble();
    var open = json['open'].toDouble();
    var close = json['close'].toDouble();
    var volume = json['volume'].toDouble();

    return Candle(date: date, high: high, low: low, open: open, close: close, volume: volume);
  }
}
