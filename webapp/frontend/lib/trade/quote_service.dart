import 'package:get_10101/common/http_client.dart';
import 'package:get_10101/common/model.dart';
import 'dart:convert';

class BestQuote {
  Price? bid;
  Price? ask;

  BestQuote({this.bid, this.ask});

  factory BestQuote.fromJson(Map<String, dynamic> json) {
    return BestQuote(
      bid: (Price.parseString(json['bid'])),
      ask: (Price.parseString(json['ask'])),
    );
  }
}

class QuoteService {
  const QuoteService();

  Future<BestQuote?> fetchQuote() async {
    final response = await HttpClientManager.instance.get(Uri(path: '/api/quotes/BtcUsd'));

    if (response.statusCode == 200) {
      var quote2 = BestQuote.fromJson(jsonDecode(response.body) as Map<String, dynamic>);

      return quote2;
    } else {
      return null;
    }
  }
}
