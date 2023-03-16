import 'dart:async';
import 'dart:convert';

import 'package:candlesticks/candlesticks.dart';
import 'package:flutter/material.dart';
import 'package:http/http.dart' as http;

class CandlestickChart extends StatefulWidget {
  const CandlestickChart({super.key});

  @override
  State<CandlestickChart> createState() => _CandlestickChartState();
}

class _CandlestickChartState extends State<CandlestickChart> {
  List<Candle> candles = [];
  bool themeIsDark = false;
  Timer? timer;

  @override
  void initState() {
    fetchCandles().then((value) {
      setState(() {
        candles = value;
      });
    });
    super.initState();
    timer = Timer.periodic(const Duration(seconds: 30), (Timer t) => getNewCandles());
  }

  Future<List<Candle>> fetchCandles() async {
    final uri = Uri.parse(
        "https://www.bitmex.com/api/v1/trade/bucketed?binSize=1m&partial=false&symbol=XBTUSD&count=1000&reverse=true");
    final res = await http.get(uri);
    return (jsonDecode(res.body) as List<dynamic>).map((e) => parse(e)).toList().toList();
  }

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      height: 300,
      child: Candlesticks(
        candles: candles,
      ),
    );
  }

  Future<void> getNewCandles() async {
    final uri = Uri.parse(
        "https://www.bitmex.com/api/v1/trade/bucketed?binSize=1m&partial=false&symbol=XBTUSD&count=1&reverse=true");
    final res = await http.get(uri);
    var list = (jsonDecode(res.body) as List<dynamic>).map((e) => parse(e)).toList().toList();
    if (list.isNotEmpty) {
      // we expect only one item to be in the list
      var item = list[0];
      if (candles[0].date.isBefore(item.date)) {
        candles.insert(0, item);
      }
    }
  }

  @override
  void dispose() {
    timer?.cancel();
    super.dispose();
  }
}

Candle parse(Map<String, dynamic> json) {
  var date = DateTime.parse(json['timestamp']);
  var high = json['high'].toDouble();
  var low = json['low'].toDouble();
  var open = json['open'].toDouble();
  var close = json['close'].toDouble();
  var volume = json['volume'].toDouble();

  return Candle(date: date, high: high, low: low, open: open, close: close, volume: volume);
}
