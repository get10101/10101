import 'dart:convert';

import 'package:get_10101/logger/logger.dart';
import 'package:http/http.dart' as http;

class FaucetService {
  /// Pay the provided invoice with lnd faucet
  Future<void> payInvoiceWithLndFaucet(String invoice) async {
    // Default to the faucet on the 10101 server, but allow to override it
    // locally if needed for dev testing
    // It's not populated in Config struct, as it's not used in production
    String faucet =
        const String.fromEnvironment("REGTEST_FAUCET", defaultValue: "http://34.32.0.52:8080");

    final data = {'payment_request': invoice};
    final encodedData = json.encode(data);

    final response = await http.post(
      Uri.parse('$faucet/lnd/v1/channels/transactions'),
      headers: <String, String>{'Content-Type': 'application/json'},
      body: encodedData,
    );

    if (response.statusCode != 200 || !response.body.contains('"payment_error":""')) {
      throw Exception("Payment failed: Received ${response.statusCode} ${response.body}");
    } else {
      logger.i("Paying invoice succeeded: ${response.body}");
    }
  }

// Pay the generated invoice with maker faucet
  Future<void> payInvoiceWithMakerFaucet(String invoice) async {
    // Default to the faucet on the 10101 server, but allow to override it
    // locally if needed for dev testing
    // It's not populated in Config struct, as it's not used in production
    String faucet = const String.fromEnvironment("REGTEST_MAKER_FAUCET",
        defaultValue: "http://34.32.0.52:80/maker/faucet");

    final response = await http.post(
      Uri.parse('$faucet/$invoice'),
    );

    logger.i("Response ${response.body}${response.statusCode}");

    if (response.statusCode != 200) {
      throw Exception("Payment failed: Received ${response.statusCode}. ${response.body}");
    } else {
      logger.i("Paying invoice succeeded: ${response.body}");
    }
  }
}
