import 'package:decimal/decimal.dart';

enum AmountDenomination { bitcoin, satoshi }

class Amount {
  Decimal _sats = Decimal.zero;

  Amount(int sats) {
    _sats = Decimal.fromInt(sats);
  }

  int get sats => _sats.toBigInt().toInt();

  double get btc => _sats.shift(-8).toDouble();

  Amount.parse(dynamic value) : _sats = Decimal.parse(value);

  Amount.zero() : _sats = Decimal.zero;

  Amount add(Amount amount) {
    return Amount(sats + amount.sats);
  }

  Amount sub(Amount amount) {
    return Amount(sats - amount.sats);
  }
}
