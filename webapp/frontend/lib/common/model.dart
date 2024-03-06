import 'package:decimal/decimal.dart';
import 'package:decimal/intl.dart';
import 'package:get_10101/common/amount_text.dart';
import 'package:intl/intl.dart';

abstract class Formattable {
  String formatted();
}

enum AmountDenomination { bitcoin, satoshi }

class Amount implements Formattable {
  Decimal _sats = Decimal.zero;

  Amount(int sats) {
    _sats = Decimal.fromInt(sats);
  }

  // TODO: this is bad for precision
  Amount.fromBtc(double btc) {
    _sats = Decimal.fromInt((btc * (100000000.0)).round());
  }

  int get sats => _sats.toBigInt().toInt();

  int get toInt => _sats.toBigInt().toInt();

  double get btc => _sats.shift(-8).toDouble();

  double asDouble() => _sats.toDouble();

  Amount.parse(dynamic value) : _sats = Decimal.parse(value);

  Amount.zero() : _sats = Decimal.zero;

  Amount add(Amount amount) {
    return Amount(sats + amount.sats);
  }

  Amount sub(Amount amount) {
    return Amount(sats - amount.sats);
  }

  Amount.parseAmount(String? value) {
    if (value == null || value.isEmpty) {
      _sats = Decimal.zero;
      return;
    }

    try {
      final f = NumberFormat("#,###");
      int amount =
          // remove any comma and dot from text formatting the users input.
          int.parse(value.replaceAll(f.symbols.GROUP_SEP, ''));

      _sats = Decimal.fromInt(amount);
    } on Exception {
      _sats = Decimal.zero;
    }
  }

  Usd operator *(Price multiplier) {
    Usd result = Usd.zero();
    result._usd = Decimal.parse((btc * multiplier.asDouble).toString());
    return result;
  }

  @override
  String formatted() {
    final formatter = NumberFormat("#,###,###,###,###", "en");
    return formatter.format(sats);
  }

  @override
  String toString() {
    return formatSats(this);
  }
}

class Usd implements Formattable {
  Decimal _usd = Decimal.zero;

  Usd(double usd) {
    _usd = Decimal.parse(usd.toString());
  }

  int get usd => _usd.toBigInt().toInt();

  int get toInt => _usd.toBigInt().toInt();

  Decimal get toDecimal => _usd;

  double get asDouble => _usd.toDouble();

  Usd.zero() : _usd = Decimal.zero;

  Usd.parse(dynamic value) : _usd = Decimal.parse(value);

  Usd.parseString(String? value) {
    if (value == null || value.isEmpty) {
      _usd = Decimal.zero;
      return;
    }

    try {
      final f = NumberFormat("#,###");
      int amount =
          // remove any comma and dot from text formatting the users input.
          int.parse(value.replaceAll(f.symbols.GROUP_SEP, ''));

      _usd = Decimal.fromInt(amount);
    } on Exception {
      _usd = Decimal.zero;
    }
  }

  @override
  String formatted() {
    final formatter = NumberFormat("#,###,###,###,###", "en");
    return formatter.format(_usd.toDouble());
  }

  @override
  String toString() {
    return formatUsd(this);
  }
}

class Price implements Formattable {
  Decimal _usd = Decimal.zero;

  Price(double usd) {
    _usd = Decimal.parse(usd.toString());
  }

  int get usd => _usd.toBigInt().toInt();

  int get toInt => _usd.toBigInt().toInt();

  Decimal get toDecimal => _usd;

  Price.zero() : _usd = Decimal.zero;

  double get asDouble => _usd.toDouble();

  Price.parse(dynamic value) : _usd = Decimal.parse(value);

  Price.parseString(String? value) {
    if (value == null || value.isEmpty) {
      _usd = Decimal.zero;
      return;
    }

    try {
      _usd = Decimal.parse(value);
    } on Exception {
      _usd = Decimal.zero;
    }
  }

  Price operator +(Price other) {
    Price result = Price.zero();
    result._usd = _usd + other._usd;
    return result;
  }

  Price operator /(Decimal divisor) {
    Price result = Price.zero();
    result._usd = (_usd / divisor).toDecimal();
    return result;
  }

  @override
  String formatted() {
    final formatter = NumberFormat("#,##0.00", "en_US");
    return formatter.format(DecimalIntl(_usd));
  }

  @override
  String toString() {
    return formatPrice(this);
  }
}

class Leverage implements Formattable {
  double _leverage = 1;

  Leverage.one() : _leverage = 1.0;

  double get asDouble => _leverage;

  Leverage(double leverage) {
    _leverage = leverage;
  }

  @override
  String formatted() {
    return _leverage.toString();
  }
}

class Quote {
  final Price _bid;
  final Price _ask;

  Price? get bid => _bid;

  Price? get ask => _ask;

  Quote(this._bid, this._ask);
}
