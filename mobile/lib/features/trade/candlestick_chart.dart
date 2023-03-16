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

  @override
  void initState() {
    fetchCandles().then((value) {
      setState(() {
        candles = value;
      });
    });
    super.initState();
  }

  Future<List<Candle>> fetchCandles() async {
    final uri = Uri.parse(
        "https://www.bitmex.com/api/v1/trade/bucketed?binSize=1m&partial=false&symbol=XBTUSD&count=1000&reverse=true");
    final res = await http.get(uri);
    return (jsonDecode(res.body) as List<dynamic>).map((e) => parse(e)).toList().reversed.toList();
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
