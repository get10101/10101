# 10101 Architecture

This is a working document to design the architecture drafted in the below diagram. It will not remain in that structure but rather will be eventually documented in ADRs.

![Assumed component diagram](http://www.plantuml.com/plantuml/proxy?cache=no&src=https://raw.githubusercontent.com/get10101/10101/d7619613324df3ce5c0f0a35d97bf24323f9515b/docs/diagrams/testing-component-diagram.puml)

The following represents a non exhaustive list of topics relevant to be discussed when designing the 10101 architecture and is primarily based on the assumed component diagram introduced in https://github.com/get10101/10101/pull/30. Please also note that the order does not represent a priority of topics.

## Database

Both app and coordinator need to persist data. This section addresses the corresponding database decisions.

### What data needs to be stored?

 - Trading relevant data, like Offers, Orders, etc.
 - Lightning channel related data.
 - DLC related data - lightning channel extension data.

Assumption: The app and coordinator hold mostly the same kind of data.

### Should we use the same database / schemas for the app and coordinator?

- **Option 1: Use the *same database***: The data relevant to be stored is structurally very similar for the app and the coordinator. In the poc (as well as in itchysats) we've used the same database abd schemas for both databases (taker and maker).
  - **Advantage**: Maintaining migrations as well as the data access layer code is simplified.

- **Option 2: Use *separate databases***: While using the same database makes sense from a maintenance point of view it might restrict us in the technology choices since a mobile app and backend service do have completely differnt constraints. It might as well hinder us from optimising our backend service as we have to consider mobile constraints.
  - **Advantage**: Databases can be optimised according to the platform specific needs.

### What kind of database should be used?

- **Option 1: Use a *relational database***: In the past we have used relational databases, promoting a clear data structure and model. A lot of care has been taken to migrate old records to adhere to data model changes. No data redundancy.
- **Option 2: Use a *no sql database***: A much more flexible option in terms of data model, with performance prioritised over data integrity. Data redundancy.


## Lightning node

Our design principles promote self-custody and self-sovereignty - as a result the users lightning node must run on her phone.

### How should the lightning node run on the phone?

- **Option 1: *Foreground process***: The lightning node will be started only when the app is opened.
  - **Advantages**: low battery consumption, and simple implementation
- **Option 2: *Background process***: The lightning node runs as background task on the phone, even if the app is closed.
  - **Advantages**: payments can be received at any time

Another alternative has been discussed if push notifications could be used to notify the user about an incoming payment asking to accept it by opening the app. We have not addressed the question of privacy.

However, this feature might not be important for the MVP.

### How do we extend the lightning channel for DLCs?

DLCs are not supported by the lightning channels outof the box and a custom extension needs to be implemented. Downwards compatibility should be granted though.

 - **Option 1: *Virtual Channels***: ..
 - **Option 2: Extend commit transaction**: see https://10101.substack.com/p/noncustodial-trading-with-10101

## State management

How are state changes managed in 10101.

 - **Option 1: *Events***: State changes are reflected with immutable events allowing to recreate a state by applying all events in the correct order. While this architectural pattern introduces some complexities it allows for a much more flexible, async processing. In itchysats an event driven architecture has been implemented for parts of the solution.
 - **Option 2: *API***: State changes are promoted through synchronous api calls with a blocking wait to check if the call has been processed successfully. This is a simple paradigm an less complex.  

*Note, these options are not exhaustive nor very general for state management. Also the target architecture could make use of a combination of both approaches. Further elaboration and discussion is required here.* 

## Coordinator API

The coordinator will have to expose an API along the p2p API for the lightning (including dlc) communication e.g. order book. **Assumption**: 10101 will not exclusively talk via the p2p protocol with the app. As DLCs should get integrated into upstream rust-lightning a more generalised approach would be beneficiary.  

- **Option 1 *RESTful API***: Simple API to expose CRUD operations through an HTTP. Broadly used and well understood.
- **Option 2 *Websocket***: Only one API, exposing all sorts of data the client can subscribe to.

A combination of both options seem to be reasonable as the approach in itchysats with only web sockets seemed artificial and introduced unneeded complexities. However using websockets to push updates to subscribed data objects (instead of polling) seems to be a good practice for multiple reasons (bandwidth, freshness, etc.).

*We also shortly discussed graphql, but probably figured it to be not beneficiary (as too powerful and complex) for our use case.*
  
### RPC vs REST: 

It seems RPC is broadly used nowadays. We might want to consider evaluating RPC for our Coordinator APIs.

## Security

One of our most important features is Security. We must not only think of it from a point of view that the protocol is secure. Equally we need to look at the way we derive and manage crypto materials. 

### How to secure the cryptographic key on the phone?

- **Option 1 *HSM***: iPhone and Android support hardware security modules on the phone. For iPhone its Secure Element - We could use such module (isolated from the os) to securily perfom cryptographic algorithms without ever exposing the private key. *Note, its unclear what that would mean for exporting the key if required* 
- **Option 2 *Encrypt keys on disk***: Simply encrypting the keys on disk e.g. in combination with an entered pin to unlock the phone.

*Note, this again not a exhaustive list on all the options for managing cryptographic keys on the phone. More work is required here.*

### How to authenticate the user 

**Assumption**: The coordinator api does not require any authentication - however we might want to ensure that it is only used from the 10101 app.

The coordinator api should though only be exposed through HTTPS.

## Backup and restore (remember punish transactions etc.)

The user needs to be able to backup not only his keys but also his channel (dlc) data so that she can restore it on another phone. 

- **Option 1 *Closing and reopening all channels and positions***: A costly, but straight forward simplified backup process. However also costly (as on chain transactions are required) and probably not feasible as the market price for a position will shift and might not get matched when reopening.   
- **Option 2 *Backup to google cloud, icloud, nas backup, SW3 Bucket, Zip File, etc.***: Also quite straight forward, but might produce race conditions (complexities) as the apps state could live on multiple phones.

## On chain monitoring 

- **Option 1 *Electrum***: Not a standard, but broadly used and accepted.
- **Option 2 *Neutrino (BIP-157)***: A bitcoin standard to run a lightweight node. (used by Breez - full nodes can be replaced (similarly how you would do it with electrum server, but with one component less))


## Open Topics:

### Deployment View
- Reverse proxy for ssl offloading
- Containerized
- VM (docker-compose, podman) + cloud conf, Kubernetes, AWS Container Services
- what envrionments?
  - Test
    - signet node (ourselves, electrum server (depends on neutrino?), blockchain explorer (esplora)?)
  - Prod
    - full node? (electrum server (depends on neutrino), blockchain explorer (esplora)?)

