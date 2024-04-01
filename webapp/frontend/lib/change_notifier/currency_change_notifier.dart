import 'package:flutter/material.dart';

enum Currency { usd, btc, sats }

extension CurrencyExtension on Currency {
  String get name {
    switch (this) {
      case Currency.sats:
        return 'Sats';
      case Currency.btc:
        return 'BTC';
      case Currency.usd:
        return 'USD';
      default:
        throw Exception('Unknown currency');
    }
  }
}

class CurrencyChangeNotifier extends ChangeNotifier {
  Currency _currency;

  CurrencyChangeNotifier(this._currency);

  Currency get currency => _currency;

  set currency(Currency value) {
    _currency = value;
    notifyListeners();
  }
}
