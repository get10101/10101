import 'dart:async';

import 'package:flutter/material.dart';
import 'package:get_10101/logger/logger.dart';
import 'package:get_10101/services/quote_service.dart';

class QuoteChangeNotifier extends ChangeNotifier {
  final QuoteService service;
  late Timer timer;

  BestQuote? _bestQuote;

  QuoteChangeNotifier(this.service) {
    _refresh();
    Timer.periodic(const Duration(seconds: 2), (timer) async {
      _refresh();
    });
  }

  void _refresh() async {
    try {
      final quote = await service.fetchQuote();
      _bestQuote = quote;

      super.notifyListeners();
    } catch (error) {
      logger.e(error);
    }
  }

  BestQuote? getBestQuote() => _bestQuote;

  @override
  void dispose() {
    super.dispose();
    timer.cancel();
  }
}
